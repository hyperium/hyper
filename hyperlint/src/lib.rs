#![feature(phase, plugin_registrar)]

#[phase(plugin, link)]
extern crate rustc;
#[phase(plugin, link)]
extern crate syntax;

use rustc::lint::{Context, LintPassObject, LintPass, LintArray};
use rustc::middle::ty::{expr_ty, ty_str, ty_ptr, ty_rptr, Ty};
use rustc::plugin::Registry;
use syntax::ast;
use syntax::attr::AttrMetaMethods;

#[plugin_registrar]
pub fn register(reg: &mut Registry) {
    reg.register_lint_pass(box Glob as LintPassObject);
    reg.register_lint_pass(box StrToString as LintPassObject);
}

declare_lint!(GLOB, Warn, "Warn if insane glob usage is not marked")

struct Glob;

impl LintPass for Glob {

    fn get_lints(&self) -> LintArray {
        lint_array!(GLOB)
    }

    fn check_view_item(&mut self, cx: &Context, view_item: &ast::ViewItem) {
        match view_item.node {
            ast::ViewItemUse(ref view_path) => {
                match view_path.node {
                    ast::ViewPathGlob(ast::Path { ref segments, .. }, _) => {
                        let path = str_path(&**segments);
                        if path == "std::prelude"
                            || path.starts_with("self::")
                            || has_glob_attr(view_item) {
                            // all's well
                        } else {
                            let m = "Insane glob usage requires #[glob = \"explanation\"]";
                            cx.span_lint(GLOB, view_item.span, m);
                        }
                    }
                    _ => ()
                }
            },
            _ => ()
        }

        fn str_path(segments: &[ast::PathSegment]) -> String {
            segments.iter().map(|s| s.identifier.as_str()).collect::<Vec<&str>>().connect("::")
        }

        fn has_glob_attr(view_item: &ast::ViewItem) -> bool {
            view_item.attrs.iter().any(|a|  a.check_name("glob") && a.value_str().is_some())
        }

    }
}

declare_lint!(STR_TO_STRING, Warn, "Warn when a String could use into_string() instead of to_string()")

struct StrToString;

impl LintPass for StrToString {
    fn get_lints(&self) -> LintArray {
        lint_array!(STR_TO_STRING)
    }

    fn check_expr(&mut self, cx: &Context, expr: &ast::Expr) {
        match expr.node {
            ast::ExprMethodCall(ref method, _, ref args)
                if method.node.as_str() == "to_string"
                && is_str(cx, &*args[0]) => {
                cx.span_lint(STR_TO_STRING, expr.span, "str.into_string() is faster");
            },
            _ => ()
        }

        fn is_str(cx: &Context, expr: &ast::Expr) -> bool {
            fn walk_ty<'t>(ty: Ty<'t>) -> Ty<'t> {
                //println!("{}: -> {}", depth, ty);
                match ty.sty {
                    ty_ptr(ref tm) | ty_rptr(_, ref tm) => walk_ty(tm.ty),
                    _ => ty
                }
            }
            match walk_ty(expr_ty(cx.tcx, expr)).sty {
                ty_str => true,
                _ => false
            }
        }
    }
}
