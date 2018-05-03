/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::codegen::Bindings::TextEncoderBinding;
use dom::bindings::codegen::Bindings::TextEncoderBinding::TextEncoderMethods;
use dom::bindings::error::Fallible;
use dom::bindings::reflector::{Reflector, reflect_dom_object};
use dom::bindings::root::DomRoot;
use dom::bindings::str::{DOMString, USVString};
use dom::globalscope::GlobalScope;
use dom_struct::dom_struct;
use js::jsapi::{JSContext, JSObject};
use js::typedarray::{Uint8Array, CreateWith};
use std::ptr;
use std::ptr::NonNull;
use typeholder::TypeHolderTrait;
use std::marker::PhantomData;

#[dom_struct]
pub struct TextEncoder<TH: TypeHolderTrait> {
    reflector_: Reflector<TH>,
    _p: PhantomData<TH>,
}

impl<TH: TypeHolderTrait> TextEncoder<TH> {
    fn new_inherited() -> TextEncoder<TH> {
        TextEncoder {
            reflector_: Reflector::new(),
            _p: Default::default(),
        }
    }

    pub fn new(global: &GlobalScope<TH>) -> DomRoot<TextEncoder<TH>> {
        reflect_dom_object(Box::new(TextEncoder::new_inherited()),
                           global,
                           TextEncoderBinding::Wrap)
    }

    // https://encoding.spec.whatwg.org/#dom-textencoder
    pub fn Constructor(global: &GlobalScope<TH>) -> Fallible<DomRoot<TextEncoder<TH>>> {
        Ok(TextEncoder::new(global))
    }
}

impl<TH: TypeHolderTrait> TextEncoderMethods for TextEncoder<TH> {
    // https://encoding.spec.whatwg.org/#dom-textencoder-encoding
    fn Encoding(&self) -> DOMString {
        DOMString::from("utf-8")
    }

    #[allow(unsafe_code)]
    // https://encoding.spec.whatwg.org/#dom-textencoder-encode
    unsafe fn Encode(&self, cx: *mut JSContext, input: USVString) -> NonNull<JSObject> {
        let encoded = input.0.as_bytes();

        rooted!(in(cx) let mut js_object = ptr::null_mut::<JSObject>());
        assert!(Uint8Array::create(cx, CreateWith::Slice(&encoded), js_object.handle_mut()).is_ok());

        NonNull::new_unchecked(js_object.get())
    }
}
