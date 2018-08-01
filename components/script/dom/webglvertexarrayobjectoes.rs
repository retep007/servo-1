/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use canvas_traits::webgl::WebGLVertexArrayId;
use dom::bindings::codegen::Bindings::WebGLVertexArrayObjectOESBinding;
use dom::bindings::reflector::{DomObject, reflect_dom_object};
use dom::bindings::root::{DomRoot, MutNullableDom};
use dom::webglbuffer::WebGLBuffer;
use dom::webglobject::WebGLObject;
use dom::webglrenderingcontext::{VertexAttribs, WebGLRenderingContext};
use dom_struct::dom_struct;
use std::cell::Cell;
use typeholder::TypeHolderTrait;

#[dom_struct]
pub struct WebGLVertexArrayObjectOES<TH: TypeHolderTrait> {
    webgl_object_: WebGLObject<TH>,
    id: WebGLVertexArrayId,
    ever_bound: Cell<bool>,
    is_deleted: Cell<bool>,
    vertex_attribs: VertexAttribs<TH>,
    bound_buffer_element_array: MutNullableDom<WebGLBuffer<TH>>,
}

impl<TH: TypeHolderTrait> WebGLVertexArrayObjectOES<TH> {
    fn new_inherited(context: &WebGLRenderingContext<TH>, id: WebGLVertexArrayId) -> Self {
        Self {
            webgl_object_: WebGLObject::new_inherited(context),
            id: id,
            ever_bound: Cell::new(false),
            is_deleted: Cell::new(false),
            vertex_attribs: VertexAttribs::new(context.limits().max_vertex_attribs),
            bound_buffer_element_array: MutNullableDom::new(None),
        }
    }

    pub fn new(context: &WebGLRenderingContext<TH>, id: WebGLVertexArrayId) -> DomRoot<Self> {
        reflect_dom_object(
            Box::new(WebGLVertexArrayObjectOES::new_inherited(context, id)),
            &*context.global(),
            WebGLVertexArrayObjectOESBinding::Wrap,
        )
    }

    pub fn vertex_attribs(&self) -> &VertexAttribs<TH> {
        &self.vertex_attribs
    }

    pub fn id(&self) -> WebGLVertexArrayId {
        self.id
    }

    pub fn is_deleted(&self) -> bool {
        self.is_deleted.get()
    }

    pub fn set_deleted(&self) {
        self.is_deleted.set(true)
    }

    pub fn ever_bound(&self) -> bool {
        return self.ever_bound.get()
    }

    pub fn set_ever_bound(&self) {
        self.ever_bound.set(true);
    }

    pub fn bound_buffer_element_array(&self) -> Option<DomRoot<WebGLBuffer<TH>>> {
        self.bound_buffer_element_array.get()
    }

    pub fn set_bound_buffer_element_array(&self, buffer: Option<&WebGLBuffer<TH>>) {
        self.bound_buffer_element_array.set(buffer);
    }
}
