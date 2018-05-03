/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::codegen::Bindings::EventBinding::EventMethods;
use dom::bindings::codegen::Bindings::MediaQueryListEventBinding;
use dom::bindings::codegen::Bindings::MediaQueryListEventBinding::MediaQueryListEventInit;
use dom::bindings::codegen::Bindings::MediaQueryListEventBinding::MediaQueryListEventMethods;
use dom::bindings::error::Fallible;
use dom::bindings::inheritance::Castable;
use dom::bindings::reflector::reflect_dom_object;
use dom::bindings::root::DomRoot;
use dom::bindings::str::DOMString;
use dom::event::Event;
use dom::globalscope::GlobalScope;
use dom::window::Window;
use dom_struct::dom_struct;
use servo_atoms::Atom;
use std::cell::Cell;
use typeholder::TypeHolderTrait;

// https://drafts.csswg.org/cssom-view/#dom-mediaquerylistevent-mediaquerylistevent
#[dom_struct]
pub struct MediaQueryListEvent<TH: TypeHolderTrait> {
    event: Event<TH>,
    media: DOMString,
    matches: Cell<bool>,
}

impl<TH: TypeHolderTrait> MediaQueryListEvent<TH> {
    pub fn new_initialized(
        global: &GlobalScope<TH>,
        media: DOMString,
        matches: bool,
    ) -> DomRoot<MediaQueryListEvent<TH>> {
        let ev = Box::new(MediaQueryListEvent {
            event: Event::new_inherited(),
            media: media,
            matches: Cell::new(matches),
        });
        reflect_dom_object(ev, global, MediaQueryListEventBinding::Wrap)
    }

    pub fn new(
        global: &GlobalScope<TH>,
        type_: Atom,
        bubbles: bool,
        cancelable: bool,
        media: DOMString,
        matches: bool,
    ) -> DomRoot<MediaQueryListEvent<TH>> {
        let ev = MediaQueryListEvent::new_initialized(global, media, matches);
        {
            let event = ev.upcast::<Event<TH>>();
            event.init_event(type_, bubbles, cancelable);
        }
        ev
    }

    pub fn Constructor(
        window: &Window<TH>,
        type_: DOMString,
        init: &MediaQueryListEventInit,
    ) -> Fallible<DomRoot<MediaQueryListEvent<TH>>> {
        let global = window.upcast::<GlobalScope<TH>>();
        Ok(MediaQueryListEvent::new(
            global,
            Atom::from(type_),
            init.parent.bubbles,
            init.parent.cancelable,
            init.media.clone(),
            init.matches,
        ))
    }
}

impl<TH: TypeHolderTrait> MediaQueryListEventMethods for MediaQueryListEvent<TH> {
    // https://drafts.csswg.org/cssom-view/#dom-mediaquerylistevent-media
    fn Media(&self) -> DOMString {
        self.media.clone()
    }

    // https://drafts.csswg.org/cssom-view/#dom-mediaquerylistevent-matches
    fn Matches(&self) -> bool {
        self.matches.get()
    }

    // https://dom.spec.whatwg.org/#dom-event-istrusted
    fn IsTrusted(&self) -> bool {
        self.upcast::<Event<TH>>().IsTrusted()
    }
}
