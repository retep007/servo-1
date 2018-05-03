/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use devtools;
use devtools_traits::DevtoolScriptControlMsg;
use dom::abstractworker::{SharedRt, SimpleWorkerErrorHandler, WorkerScriptMsg};
use dom::abstractworkerglobalscope::{SendableWorkerScriptChan, WorkerThreadWorkerChan};
use dom::bindings::cell::DomRefCell;
use dom::bindings::codegen::Bindings::DedicatedWorkerGlobalScopeBinding;
use dom::bindings::codegen::Bindings::DedicatedWorkerGlobalScopeBinding::DedicatedWorkerGlobalScopeMethods;
use dom::bindings::error::{ErrorInfo, ErrorResult};
use dom::bindings::inheritance::Castable;
use dom::bindings::reflector::DomObject;
use dom::bindings::root::{DomRoot, RootCollection, ThreadLocalStackRoots};
use dom::bindings::str::DOMString;
use dom::bindings::structuredclone::StructuredCloneData;
use dom::errorevent::ErrorEvent;
use dom::event::{Event, EventBubbles, EventCancelable, EventStatus};
use dom::eventtarget::EventTarget;
use dom::globalscope::GlobalScope;
use dom::messageevent::MessageEvent;
use dom::worker::{TrustedWorkerAddress, Worker};
use dom::workerglobalscope::WorkerGlobalScope;
use dom_struct::dom_struct;
use ipc_channel::ipc::{self, IpcReceiver, IpcSender};
use ipc_channel::router::ROUTER;
use js::jsapi::{JS_SetInterruptCallback, JSAutoCompartment, JSContext};
use js::jsval::UndefinedValue;
use js::rust::HandleValue;
use msg::constellation_msg::TopLevelBrowsingContextId;
use net_traits::{IpcSend, load_whole_resource};
use net_traits::request::{CredentialsMode, Destination, RequestInit};
use script_runtime::{CommonScriptMsg, ScriptChan, ScriptPort, new_rt_and_cx, Runtime};
use script_runtime::ScriptThreadEventCategory::WorkerEvent;
use script_traits::{TimerEvent, TimerSource, WorkerGlobalScopeInit, WorkerScriptLoadOrigin};
use servo_rand::random;
use servo_url::ServoUrl;
use std::mem::replace;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, RecvError, Select, Sender, channel};
use std::thread;
use style::thread_state::{self, ThreadState};
use typeholder::TypeHolderTrait;

/// Set the `worker` field of a related DedicatedWorkerGlobalScope object to a particular
/// value for the duration of this object's lifetime. This ensures that the related Worker
/// object only lives as long as necessary (ie. while events are being executed), while
/// providing a reference that can be cloned freely.
struct AutoWorkerReset<'a, TH: TypeHolderTrait> {
    workerscope: &'a DedicatedWorkerGlobalScope<TH>,
    old_worker: Option<TrustedWorkerAddress<TH>>,
}

impl<'a, TH: TypeHolderTrait> AutoWorkerReset<'a, TH> {
    fn new(workerscope: &'a DedicatedWorkerGlobalScope<TH>,
           worker: TrustedWorkerAddress<TH>)
           -> AutoWorkerReset<'a, TH> {
        AutoWorkerReset {
            workerscope: workerscope,
            old_worker: replace(&mut *workerscope.worker.borrow_mut(), Some(worker)),
        }
    }
}

impl<'a, TH: TypeHolderTrait> Drop for AutoWorkerReset<'a, TH> {
    fn drop(&mut self) {
        *self.workerscope.worker.borrow_mut() = self.old_worker.clone();
    }
}

enum MixedMessage<TH: TypeHolderTrait> {
    FromWorker((TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)),
    FromScheduler((TrustedWorkerAddress<TH>, TimerEvent)),
    FromDevtools(DevtoolScriptControlMsg)
}

// https://html.spec.whatwg.org/multipage/#dedicatedworkerglobalscope
#[dom_struct]
pub struct DedicatedWorkerGlobalScope<TH: TypeHolderTrait>
 {
    workerglobalscope: WorkerGlobalScope<TH>,
    #[ignore_malloc_size_of = "Defined in std"]
    receiver: Receiver<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
    #[ignore_malloc_size_of = "Defined in std"]
    own_sender: Sender<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
    #[ignore_malloc_size_of = "Defined in std"]
    timer_event_port: Receiver<(TrustedWorkerAddress<TH>, TimerEvent)>,
    #[ignore_malloc_size_of = "Trusted<T> has unclear ownership like Dom<T>"]
    worker: DomRefCell<Option<TrustedWorkerAddress<TH>>>,
    #[ignore_malloc_size_of = "Can't measure trait objects"]
    /// Sender to the parent thread.
    parent_sender: Box<ScriptChan + Send>,
}

impl<TH: TypeHolderTrait> DedicatedWorkerGlobalScope<TH> {
    fn new_inherited(init: WorkerGlobalScopeInit,
                     worker_url: ServoUrl,
                     from_devtools_receiver: Receiver<DevtoolScriptControlMsg>,
                     runtime: Runtime,
                     parent_sender: Box<ScriptChan + Send>,
                     own_sender: Sender<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
                     receiver: Receiver<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
                     timer_event_chan: IpcSender<TimerEvent>,
                     timer_event_port: Receiver<(TrustedWorkerAddress<TH>, TimerEvent)>,
                     closing: Arc<AtomicBool>)
                     -> DedicatedWorkerGlobalScope<TH> {
        DedicatedWorkerGlobalScope {
            workerglobalscope: WorkerGlobalScope::new_inherited(init,
                                                                worker_url,
                                                                runtime,
                                                                from_devtools_receiver,
                                                                timer_event_chan,
                                                                Some(closing)),
            receiver: receiver,
            own_sender: own_sender,
            timer_event_port: timer_event_port,
            parent_sender: parent_sender,
            worker: DomRefCell::new(None),
        }
    }

    #[allow(unsafe_code)]
    pub fn new(init: WorkerGlobalScopeInit,
               worker_url: ServoUrl,
               from_devtools_receiver: Receiver<DevtoolScriptControlMsg>,
               runtime: Runtime,
               parent_sender: Box<ScriptChan + Send>,
               own_sender: Sender<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
               receiver: Receiver<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
               timer_event_chan: IpcSender<TimerEvent>,
               timer_event_port: Receiver<(TrustedWorkerAddress<TH>, TimerEvent)>,
               closing: Arc<AtomicBool>)
               -> DomRoot<DedicatedWorkerGlobalScope<TH>> {
        let cx = runtime.cx();
        let scope = Box::new(DedicatedWorkerGlobalScope::new_inherited(
            init,
            worker_url,
            from_devtools_receiver,
            runtime,
            parent_sender,
            own_sender,
            receiver,
            timer_event_chan,
            timer_event_port,
            closing
        ));
        unsafe {
            DedicatedWorkerGlobalScopeBinding::Wrap(cx, scope)
        }
    }

    #[allow(unsafe_code)]
    pub fn run_worker_scope(init: WorkerGlobalScopeInit,
                            worker_url: ServoUrl,
                            from_devtools_receiver: IpcReceiver<DevtoolScriptControlMsg>,
                            worker_rt_for_mainthread: Arc<Mutex<Option<SharedRt>>>,
                            worker: TrustedWorkerAddress<TH>,
                            parent_sender: Box<ScriptChan + Send>,
                            own_sender: Sender<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
                            receiver: Receiver<(TrustedWorkerAddress<TH>, WorkerScriptMsg<TH>)>,
                            worker_load_origin: WorkerScriptLoadOrigin,
                            closing: Arc<AtomicBool>) {
        let serialized_worker_url = worker_url.to_string();
        let name = format!("WebWorker for {}", serialized_worker_url);
        let top_level_browsing_context_id = TopLevelBrowsingContextId::installed();
        let origin = GlobalScope::<TH>::current().expect("No current global object").origin().immutable().clone();

        thread::Builder::new().name(name).spawn(move || {
            thread_state::initialize(ThreadState::SCRIPT | ThreadState::IN_WORKER);

            if let Some(top_level_browsing_context_id) = top_level_browsing_context_id {
                TopLevelBrowsingContextId::install(top_level_browsing_context_id);
            }

            let roots = RootCollection::new();
            let _stack_roots = ThreadLocalStackRoots::new(&roots);

            let WorkerScriptLoadOrigin { referrer_url, referrer_policy, pipeline_id } = worker_load_origin;

            let request = RequestInit {
                url: worker_url.clone(),
                destination: Destination::Worker,
                credentials_mode: CredentialsMode::Include,
                use_url_credentials: true,
                pipeline_id: pipeline_id,
                referrer_url: referrer_url,
                referrer_policy: referrer_policy,
                origin,
                .. RequestInit::default()
            };

            let (metadata, bytes) = match load_whole_resource(request,
                                                              &init.resource_threads.sender()) {
                Err(_) => {
                    println!("error loading script {}", serialized_worker_url);
                    parent_sender.send(CommonScriptMsg::Task(
                        WorkerEvent,
                        Box::new(SimpleWorkerErrorHandler::new(worker)),
                        pipeline_id
                    )).unwrap();
                    return;
                }
                Ok((metadata, bytes)) => (metadata, bytes)
            };
            let url = metadata.final_url;
            let source = String::from_utf8_lossy(&bytes);

            let runtime = unsafe { new_rt_and_cx::<TH>() };
            *worker_rt_for_mainthread.lock().unwrap() = Some(SharedRt::new(&runtime));

            let (devtools_mpsc_chan, devtools_mpsc_port) = channel();
            ROUTER.route_ipc_receiver_to_mpsc_sender(from_devtools_receiver, devtools_mpsc_chan);

            let (timer_tx, timer_rx) = channel();
            let (timer_ipc_chan, timer_ipc_port) = ipc::channel().unwrap();
            let worker_for_route = worker.clone();
            ROUTER.add_route(timer_ipc_port.to_opaque(), Box::new(move |message| {
                let event = message.to().unwrap();
                timer_tx.send((worker_for_route.clone(), event)).unwrap();
            }));

            let global = DedicatedWorkerGlobalScope::new(
                init, url, devtools_mpsc_port, runtime,
                parent_sender.clone(), own_sender, receiver,
                timer_ipc_chan, timer_rx, closing);
            // FIXME(njn): workers currently don't have a unique ID suitable for using in reporter
            // registration (#6631), so we instead use a random number and cross our fingers.
            let scope = global.upcast::<WorkerGlobalScope<TH>>();

            unsafe {
                // Handle interrupt requests
                JS_SetInterruptCallback(scope.runtime(), Some(interrupt_callback::<TH>));
            }

            if scope.is_closing() {
                return;
            }

            {
                let _ar = AutoWorkerReset::new(&global, worker.clone());
                scope.execute_script(DOMString::from(source));
            }

            let reporter_name = format!("dedicated-worker-reporter-{}", random::<u64>());
            scope.upcast::<GlobalScope<TH>>().mem_profiler_chan().run_with_memory_reporting(|| {
                // https://html.spec.whatwg.org/multipage/#event-loop-processing-model
                // Step 1
                while let Ok(event) = global.receive_event() {
                    if scope.is_closing() {
                        break;
                    }
                    // Step 3
                    global.handle_event(event);
                    // Step 6
                    let _ar = AutoWorkerReset::new(&global, worker.clone());
                    global.upcast::<GlobalScope<TH>>().perform_a_microtask_checkpoint();
                }
            }, reporter_name, parent_sender, CommonScriptMsg::CollectReports);
        }).expect("Thread spawning failed");
    }

    pub fn script_chan(&self) -> Box<ScriptChan + Send> {
        Box::new(WorkerThreadWorkerChan {
            sender: self.own_sender.clone(),
            worker: self.worker.borrow().as_ref().unwrap().clone(),
        })
    }

    pub fn new_script_pair(&self) -> (Box<ScriptChan + Send>, Box<ScriptPort + Send>) {
        let (tx, rx) = channel();
        let chan = Box::new(SendableWorkerScriptChan {
            sender: tx,
            worker: self.worker.borrow().as_ref().unwrap().clone(),
        });
        (chan, Box::new(rx))
    }

    #[allow(unsafe_code)]
    fn receive_event(&self) -> Result<MixedMessage<TH>, RecvError> {
        let scope = self.upcast::<WorkerGlobalScope<TH>>();
        let worker_port = &self.receiver;
        let timer_event_port = &self.timer_event_port;
        let devtools_port = scope.from_devtools_receiver();

        let sel = Select::new();
        let mut worker_handle = sel.handle(worker_port);
        let mut timer_event_handle = sel.handle(timer_event_port);
        let mut devtools_handle = sel.handle(devtools_port);
        unsafe {
            worker_handle.add();
            timer_event_handle.add();
            if scope.from_devtools_sender().is_some() {
                devtools_handle.add();
            }
        }
        let ret = sel.wait();
        if ret == worker_handle.id() {
            Ok(MixedMessage::FromWorker(worker_port.recv()?))
        } else if ret == timer_event_handle.id() {
            Ok(MixedMessage::FromScheduler(timer_event_port.recv()?))
        } else if ret == devtools_handle.id() {
            Ok(MixedMessage::FromDevtools(devtools_port.recv()?))
        } else {
            panic!("unexpected select result!")
        }
    }

    fn handle_script_event(&self, msg: WorkerScriptMsg<TH>) {
        match msg {
            WorkerScriptMsg::DOMMessage(data) => {
                let scope = self.upcast::<WorkerGlobalScope<TH>>();
                let target = self.upcast();
                let _ac = JSAutoCompartment::new(scope.get_cx(),
                                                 scope.reflector().get_jsobject().get());
                rooted!(in(scope.get_cx()) let mut message = UndefinedValue());
                data.read(scope.upcast(), message.handle_mut());
                MessageEvent::dispatch_jsval(target, scope.upcast(), message.handle());
            },
            WorkerScriptMsg::Common(msg) => {
                self.upcast::<WorkerGlobalScope<TH>>().process_event(msg);
            },
        }
    }

    fn handle_event(&self, event: MixedMessage<TH>) {
        match event {
            MixedMessage::FromDevtools(msg) => {
                match msg {
                    DevtoolScriptControlMsg::EvaluateJS(_pipe_id, string, sender) =>
                        devtools::handle_evaluate_js(self.upcast(), string, sender),
                    DevtoolScriptControlMsg::GetCachedMessages(pipe_id, message_types, sender) =>
                        devtools::handle_get_cached_messages(pipe_id, message_types, sender),
                    DevtoolScriptControlMsg::WantsLiveNotifications(_pipe_id, bool_val) =>
                        devtools::handle_wants_live_notifications(self.upcast(), bool_val),
                    _ => debug!("got an unusable devtools control message inside the worker!"),
                }
            },
            MixedMessage::FromScheduler((linked_worker, timer_event)) => {
                match timer_event {
                    TimerEvent(TimerSource::FromWorker, id) => {
                        let _ar = AutoWorkerReset::new(self, linked_worker);
                        let scope = self.upcast::<WorkerGlobalScope<TH>>();
                        scope.handle_fire_timer(id);
                    },
                    TimerEvent(_, _) => {
                        panic!("A worker received a TimerEvent from a window.")
                    }
                }
            }
            MixedMessage::FromWorker((linked_worker, msg)) => {
                let _ar = AutoWorkerReset::new(self, linked_worker);
                self.handle_script_event(msg);
            }
        }
    }

    // https://html.spec.whatwg.org/multipage/#runtime-script-errors-2
    #[allow(unsafe_code)]
    pub fn forward_error_to_worker_object(&self, error_info: ErrorInfo) {
        let worker = self.worker.borrow().as_ref().unwrap().clone();
        let pipeline_id = self.upcast::<GlobalScope<TH>>().pipeline_id();
        let task = Box::new(task!(forward_error_to_worker_object: move || {
            let worker = worker.root();
            let global = worker.global();

            // Step 1.
            let event = ErrorEvent::new(
                &global,
                atom!("error"),
                EventBubbles::DoesNotBubble,
                EventCancelable::Cancelable,
                error_info.message.as_str().into(),
                error_info.filename.as_str().into(),
                error_info.lineno,
                error_info.column,
                HandleValue::null(),
            );
            let event_status =
                event.upcast::<Event<TH>>().fire(worker.upcast::<EventTarget<TH>>());

            // Step 2.
            if event_status == EventStatus::NotCanceled {
                global.report_an_error(error_info, HandleValue::null());
            }
        }));
        // TODO: Should use the DOM manipulation task source.
        self.parent_sender.send(CommonScriptMsg::Task(WorkerEvent, task, Some(pipeline_id))).unwrap();
    }
}

#[allow(unsafe_code)]
unsafe extern "C" fn interrupt_callback<TH: TypeHolderTrait>(cx: *mut JSContext) -> bool {
    let worker =
        DomRoot::downcast::<WorkerGlobalScope<TH>>(GlobalScope::<TH>::from_context(cx))
            .expect("global is not a worker scope");
    assert!(worker.is::<DedicatedWorkerGlobalScope<TH>>());

    // A false response causes the script to terminate
    !worker.is_closing()
}

impl<TH: TypeHolderTrait> DedicatedWorkerGlobalScopeMethods<TH> for DedicatedWorkerGlobalScope<TH> {
    #[allow(unsafe_code)]
    // https://html.spec.whatwg.org/multipage/#dom-dedicatedworkerglobalscope-postmessage
    unsafe fn PostMessage(&self, cx: *mut JSContext, message: HandleValue) -> ErrorResult {
        let data = StructuredCloneData::write(cx, message)?;
        let worker = self.worker.borrow().as_ref().unwrap().clone();
        let pipeline_id = self.upcast::<GlobalScope<TH>>().pipeline_id();
        let task = Box::new(task!(post_worker_message: move || {
            Worker::handle_message(worker, data);
        }));
        self.parent_sender.send(CommonScriptMsg::Task(WorkerEvent, task, Some(pipeline_id))).unwrap();
        Ok(())
    }

    // https://html.spec.whatwg.org/multipage/#dom-dedicatedworkerglobalscope-close
    fn Close(&self) {
        // Step 2
        self.upcast::<WorkerGlobalScope<TH>>().close();
    }

    // https://html.spec.whatwg.org/multipage/#handler-dedicatedworkerglobalscope-onmessage
    event_handler!(message, GetOnmessage, SetOnmessage);
}
