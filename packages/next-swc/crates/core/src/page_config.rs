use chrono::Utc;
use swc_common::{Span, DUMMY_SP};
use swc_ecmascript::ast::*;
use swc_ecmascript::utils::HANDLER;
use swc_ecmascript::visit::{Fold, FoldWith};

pub fn page_config(is_development: bool, is_page_file: bool) -> impl Fold {
  PageConfig {
    is_development,
    is_page_file,
    ..Default::default()
  }
}

pub fn page_config_test() -> impl Fold {
  PageConfig {
    in_test: true,
    is_page_file: true,
    ..Default::default()
  }
}

#[derive(Debug, Default)]
struct PageConfig {
  drop_bundle: bool,
  in_test: bool,
  is_development: bool,
  is_page_file: bool,
}

const STRING_LITERAL_DROP_BUNDLE: &str = "__NEXT_DROP_CLIENT_FILE__";
const CONFIG_KEY: &str = "config";

impl Fold for PageConfig {
  fn fold_module_items(&mut self, items: Vec<ModuleItem>) -> Vec<ModuleItem> {
    let mut new_items = vec![];
    for item in items {
      new_items.push(item.fold_with(self));
      if !self.is_development && self.drop_bundle {
        let timestamp = match self.in_test {
          true => String::from("mock_timestamp"),
          false => Utc::now().timestamp().to_string(),
        };
        return vec![ModuleItem::Stmt(Stmt::Decl(Decl::Var(VarDecl {
          decls: vec![VarDeclarator {
            name: Pat::Ident(BindingIdent {
              id: Ident {
                sym: STRING_LITERAL_DROP_BUNDLE.into(),
                span: DUMMY_SP,
                optional: false,
              },
              type_ann: None,
            }),
            init: Some(Box::new(Expr::Lit(Lit::Str(Str {
              value: format!("{} {}", STRING_LITERAL_DROP_BUNDLE, timestamp).into(),
              span: DUMMY_SP,
              kind: StrKind::Synthesized {},
              has_escape: false,
            })))),
            span: DUMMY_SP,
            definite: false,
          }],
          span: DUMMY_SP,
          kind: VarDeclKind::Const,
          declare: false,
        })))];
      }
    }

    new_items
  }

  fn fold_export_decl(&mut self, export: ExportDecl) -> ExportDecl {
    match &export.decl {
      Decl::Var(var_decl) => {
        for decl in &var_decl.decls {
          let mut is_config = false;
          if let Pat::Ident(ident) = &decl.name {
            if &ident.id.sym == CONFIG_KEY {
              is_config = true;
            }
          }

          if is_config {
            if let Some(expr) = &decl.init {
              if let Expr::Object(obj) = &**expr {
                for prop in &obj.props {
                  if let PropOrSpread::Prop(prop) = prop {
                    if let Prop::KeyValue(kv) = &**prop {
                      match &kv.key {
                        PropName::Ident(ident) => {
                          if &ident.sym == "amp" {
                            if let Expr::Lit(Lit::Bool(Bool { value, .. })) = &*kv.value {
                              if *value && self.is_page_file {
                                self.drop_bundle = true;
                              }
                            } else if let Expr::Lit(Lit::Str(_)) = &*kv.value {
                              // Do not replace bundle
                            } else {
                              self.handle_error("Invalid value found.", export.span);
                            }
                          }
                        }
                        _ => {
                          self.handle_error("Invalid property found.", export.span);
                        }
                      }
                    } else {
                      self.handle_error("Invalid property or value.", export.span);
                    }
                  } else {
                    self.handle_error("Property spread is not allowed.", export.span);
                  }
                }
              } else {
                self.handle_error("Expected config to be an object.", export.span);
              }
            } else {
              self.handle_error("Expected config to be an object.", export.span);
            }
          }
        }
      }
      _ => {}
    }
    export
  }

  fn fold_export_named_specifier(
    &mut self,
    specifier: ExportNamedSpecifier,
  ) -> ExportNamedSpecifier {
    match &specifier.exported {
      Some(ident) => {
        if &ident.sym == CONFIG_KEY {
          self.handle_error("Config cannot be re-exported.", specifier.span)
        }
      }
      None => {
        if &specifier.orig.sym == CONFIG_KEY {
          self.handle_error("Config cannot be re-exported.", specifier.span)
        }
      }
    }
    specifier
  }
}

impl PageConfig {
  fn handle_error(&mut self, details: &str, span: Span) {
    if self.is_page_file {
      let message = format!("Invalid page config export found. {} \
      See: https://nextjs.org/docs/messages/invalid-page-config", details);
      HANDLER.with(|handler| handler.struct_span_err(span, &message).emit());
    }
  }
}
