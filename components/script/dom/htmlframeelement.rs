/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::codegen::Bindings::HTMLFrameElementBinding;
use dom::bindings::root::DomRoot;
use dom::document::Document;
use dom::htmlelement::HTMLElement;
use dom::node::Node;
use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use typeholder::TypeHolderTrait;

#[dom_struct]
pub struct HTMLFrameElement<TH: TypeHolderTrait> {
    htmlelement: HTMLElement<TH>
}

impl<TH: TypeHolderTrait> HTMLFrameElement<TH> {
    fn new_inherited(local_name: LocalName, prefix: Option<Prefix>, document: &Document<TH>) -> HTMLFrameElement<TH> {
        HTMLFrameElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document)
        }
    }

    #[allow(unrooted_must_root)]
    pub fn new(local_name: LocalName,
               prefix: Option<Prefix>,
               document: &Document<TH>) -> DomRoot<HTMLFrameElement<TH>> {
        Node::<TH>::reflect_node(Box::new(HTMLFrameElement::new_inherited(local_name, prefix, document)),
                           document,
                           HTMLFrameElementBinding::Wrap)
    }
}
