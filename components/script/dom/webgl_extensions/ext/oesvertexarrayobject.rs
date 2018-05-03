/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use canvas_traits::webgl::{webgl_channel, WebGLCommand, WebGLError, WebGLVersion};
use dom::bindings::codegen::Bindings::OESVertexArrayObjectBinding::{self, OESVertexArrayObjectMethods};
use dom::bindings::codegen::Bindings::OESVertexArrayObjectBinding::OESVertexArrayObjectConstants;
use dom::bindings::reflector::{DomObject, Reflector, reflect_dom_object};
use dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use dom::webglrenderingcontext::WebGLRenderingContext;
use dom::webglvertexarrayobjectoes::WebGLVertexArrayObjectOES;
use dom_struct::dom_struct;
use js::conversions::ToJSValConvertible;
use js::jsapi::JSContext;
use js::jsval::{JSVal, NullValue};
use super::{WebGLExtension, WebGLExtensions, WebGLExtensionSpec};
use typeholder::TypeHolderTrait;

#[dom_struct]
pub struct OESVertexArrayObject<TH: TypeHolderTrait> {
    reflector_: Reflector<TH>,
    ctx: Dom<WebGLRenderingContext<TH>>,
    bound_vao: MutNullableDom<WebGLVertexArrayObjectOES<TH>>,
}

impl<TH: TypeHolderTrait> OESVertexArrayObject<TH> {
    fn new_inherited(ctx: &WebGLRenderingContext<TH>) -> OESVertexArrayObject<TH> {
        Self {
            reflector_: Reflector::new(),
            ctx: Dom::from_ref(ctx),
            bound_vao: MutNullableDom::new(None)
        }
    }

    #[allow(unsafe_code)]
    fn get_current_binding(&self, cx:*mut JSContext) -> JSVal {
        rooted!(in(cx) let mut rval = NullValue());
        if let Some(bound_vao) = self.bound_vao.get() {
            unsafe {
                bound_vao.to_jsval(cx, rval.handle_mut());
            }
        }
        rval.get()
    }
}

impl<TH: TypeHolderTrait> OESVertexArrayObjectMethods<TH> for OESVertexArrayObject<TH> {
    // https://www.khronos.org/registry/webgl/extensions/OES_vertex_array_object/
    fn CreateVertexArrayOES(&self) -> Option<DomRoot<WebGLVertexArrayObjectOES<TH>>> {
        let (sender, receiver) = webgl_channel().unwrap();
        self.ctx.send_command(WebGLCommand::CreateVertexArray(sender));
        receiver.recv().unwrap().map(|id| WebGLVertexArrayObjectOES::new(&self.ctx, id))
    }

    // https://www.khronos.org/registry/webgl/extensions/OES_vertex_array_object/
    fn DeleteVertexArrayOES(&self, vao: Option<&WebGLVertexArrayObjectOES<TH>>) {
        if let Some(vao) = vao {
            if vao.is_deleted() {
                return;
            }

            // Unbind deleted VAO if currently bound
            if let Some(bound_vao) = self.bound_vao.get() {
                if bound_vao.id() == vao.id() {
                    self.bound_vao.set(None);
                    self.ctx.send_command(WebGLCommand::BindVertexArray(None));
                }
            }

            // Remove VAO references from buffers
            for attrib_data in &*vao.vertex_attribs().borrow() {
                if let Some(buffer) = attrib_data.buffer() {
                    buffer.remove_vao_reference(vao.id());
                }
            }
            if let Some(buffer) = vao.bound_buffer_element_array() {
                buffer.remove_vao_reference(vao.id());
            }

            // Delete the vao
            self.ctx.send_command(WebGLCommand::DeleteVertexArray(vao.id()));
            vao.set_deleted();
        }
    }

    // https://www.khronos.org/registry/webgl/extensions/OES_vertex_array_object/
    fn IsVertexArrayOES(&self, vao: Option<&WebGLVertexArrayObjectOES<TH>>) -> bool {
        // Conformance tests expect false if vao never bound
        vao.map_or(false, |vao| !vao.is_deleted() && vao.ever_bound())
    }

    // https://www.khronos.org/registry/webgl/extensions/OES_vertex_array_object/
    fn BindVertexArrayOES(&self, vao: Option<&WebGLVertexArrayObjectOES<TH>>) {
        if let Some(bound_vao) = self.bound_vao.get() {
            // Store buffers attached to attrib pointers
            bound_vao.vertex_attribs().clone_from(&self.ctx.vertex_attribs());
            for attrib_data in &*bound_vao.vertex_attribs().borrow() {
                if let Some(buffer) = attrib_data.buffer() {
                    buffer.add_vao_reference(bound_vao.id());
                }
            }
            // Store element array buffer
            let element_array = self.ctx.bound_buffer_element_array();
            bound_vao.set_bound_buffer_element_array(element_array.as_ref().map(|buffer| {
                buffer.add_vao_reference(bound_vao.id());
                &**buffer
            }));
        }

        if let Some(vao) = vao {
            if vao.is_deleted() {
                self.ctx.webgl_error(WebGLError::InvalidOperation);
                return;
            }

            self.ctx.send_command(WebGLCommand::BindVertexArray(Some(vao.id())));
            vao.set_ever_bound();
            self.bound_vao.set(Some(&vao));

            // Restore WebGLRenderingContext current bindings
            self.ctx.vertex_attribs().clone_from(&vao.vertex_attribs());
            let element_array = vao.bound_buffer_element_array();
            self.ctx.set_bound_buffer_element_array(element_array.as_ref().map(|buffer| &**buffer));
        } else {
            self.ctx.send_command(WebGLCommand::BindVertexArray(None));
            self.bound_vao.set(None);
            self.ctx.vertex_attribs().clear();
        }
    }
}

impl<TH: TypeHolderTrait> WebGLExtension<TH> for OESVertexArrayObject<TH> {
    type Extension = OESVertexArrayObject<TH>;
    fn new(ctx: &WebGLRenderingContext<TH>) -> DomRoot<OESVertexArrayObject<TH>> {
        reflect_dom_object(Box::new(OESVertexArrayObject::new_inherited(ctx)),
                           &*ctx.global(),
                           OESVertexArrayObjectBinding::Wrap)
    }

    fn spec() -> WebGLExtensionSpec {
        WebGLExtensionSpec::Specific(WebGLVersion::WebGL1)
    }

    fn is_supported(ext: &WebGLExtensions<TH>) -> bool {
        ext.supports_any_gl_extension(&["GL_OES_vertex_array_object",
                                        "GL_ARB_vertex_array_object",
                                        "GL_APPLE_vertex_array_object"])
    }

    fn enable(ext: &WebGLExtensions<TH>) {
        let query = OESVertexArrayObjectConstants::VERTEX_ARRAY_BINDING_OES;
        ext.add_query_parameter_handler(query, Box::new(|cx, webgl_ctx| {
            match webgl_ctx.get_extension_manager().get_dom_object::<OESVertexArrayObject<TH>>() {
                Some(dom_object) => {
                    Ok(dom_object.get_current_binding(cx))
                },
                None => {
                    // Extension instance not found!
                    Err(WebGLError::InvalidOperation)
                }
            }
        }));
    }

    fn name() -> &'static str {
        "OES_vertex_array_object"
    }
}
