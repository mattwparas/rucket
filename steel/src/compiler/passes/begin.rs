use crate::parser::tokens::TokenType;
use crate::parser::{
    ast::{Atom, Begin, Define, ExprKind, LambdaFunction, List, Set},
    parser::SyntaxObject,
};
use std::collections::HashSet;

use super::{Folder, VisitorMutUnit};

struct FlattenBegin {}
impl FlattenBegin {
    fn flatten(expr: ExprKind) -> ExprKind {
        FlattenBegin {}.visit(expr)
    }
}

impl Folder for FlattenBegin {
    #[inline]
    fn visit_begin(&mut self, begin: Begin) -> ExprKind {
        let span = begin.location;

        // Flatten begins
        let flattened_exprs = begin
            .exprs
            .into_iter()
            .map(|x| {
                if let ExprKind::Begin(b) = x {
                    b.exprs
                        .into_iter()
                        .map(|x| self.visit(x))
                        .collect::<Vec<_>>()
                } else {
                    vec![x]
                }
            })
            .flatten()
            .collect::<Vec<_>>();

        ExprKind::Begin(Begin::new(flattened_exprs, span))
    }
}

pub fn flatten_begins_and_expand_defines(exprs: Vec<ExprKind>) -> Vec<ExprKind> {
    exprs
        .into_iter()
        .map(|x| FlattenBegin::flatten(x))
        .map(|x| ConvertDefinesToLets::convert_defines(x))
        .collect()
}

struct MergeDefines {
    referenced_identifiers: HashSet<String>,
}

impl MergeDefines {
    fn new() -> Self {
        MergeDefines {
            referenced_identifiers: HashSet::new(),
        }
    }

    fn insert(&mut self, value: &str) {
        self.referenced_identifiers.insert(value.to_string());
    }

    fn get(&mut self, value: &str) -> Option<&str> {
        self.referenced_identifiers.get(value).map(|x| x.as_str())
    }
}

impl VisitorMutUnit for MergeDefines {
    fn visit_atom(&mut self, a: &Atom) {
        if let TokenType::Identifier(ident) = &a.syn.ty {
            self.referenced_identifiers.insert(ident.clone());
        }
    }
}

struct DefinedVars<'a> {
    defined_identifiers: &'a [&'a str],
}

impl<'a> DefinedVars<'a> {
    fn new(defined_identifiers: &'a [&'a str]) -> Self {
        DefinedVars {
            defined_identifiers,
        }
    }
}

struct ConvertDefinesToLets {
    depth: usize,
}

impl ConvertDefinesToLets {
    fn new() -> Self {
        Self { depth: 0 }
    }

    fn convert_defines(expr: ExprKind) -> ExprKind {
        ConvertDefinesToLets::new().visit(expr)
    }
}

impl Folder for ConvertDefinesToLets {
    #[inline]
    fn visit_lambda_function(&mut self, mut lambda_function: Box<LambdaFunction>) -> ExprKind {
        self.depth += 1;
        lambda_function.body = self.visit(lambda_function.body);
        self.depth -= 1;
        ExprKind::LambdaFunction(lambda_function)
    }

    // TODO
    #[inline]
    fn visit_begin(&mut self, begin: Begin) -> ExprKind {
        if self.depth > 0 {
            match convert_exprs_to_let(begin) {
                ExprKind::Begin(mut b) => {
                    b.exprs = b.exprs.into_iter().map(|e| self.visit(e)).collect();
                    ExprKind::Begin(b)
                }
                ExprKind::List(mut l) => {
                    l.args = l.args.into_iter().map(|x| self.visit(x)).collect();
                    ExprKind::List(l)
                }
                _ => panic!("Something went wrong in define conversion"),
            }
        } else {
            ExprKind::Begin(begin)
        }
    }
}

// Want to take the highest precedence form. For each of these, we can fold
// the expressions into themselves. If theres a mutual reference, turn everything into a letrec form.
// If there are no functions with no mutual references,
enum IdentifierReferenceType {
    // A function references itself - forced to be a letrec
    FuncSelfReference,
    // A function references another variable defined in the scope - combine with letrec
    FuncMutualReference,
    // A variable references no other variable in the scope - normal let
    FlatNoReference,
    // A variable references a variable defined prior in the scope - coalesce with prev define if possible
    FlatPriorReference,
}

// Snag the names from the defines for the current (flattened) begin statement
fn collect_defines_from_current_scope<'a>(begin_exprs: &'a [ExprKind]) -> Vec<(usize, &'a str)> {
    begin_exprs
        .iter()
        .enumerate()
        .filter_map(|x| {
            if let ExprKind::Define(d) = x.1 {
                let name = d
                    .name
                    .atom_identifier_or_else(|| {})
                    .expect("Define without a legal name");
                Some((x.0, name))
            } else {
                None
            }
        })
        .collect()
}

// enum ExprTypeState {
//     FlatDefineToLet(ExprKind),
//     FlatDefineToLetStar(ExprKind),
//     LetRecSelfRef(ExprKind),
//     LetRecMutualRef(ExprKind),
// }

// See if we need to keep track of any of the local variables
fn merge_defines(exprs: Vec<ExprKind>) -> Begin {
    let mut let_rec_exprs: Vec<ExprKind> = Vec::new();

    let defines = collect_defines_from_current_scope(&exprs);

    for expr in &exprs {
        match expr {
            ExprKind::Define(d) => {
                let name = d
                    .name
                    .atom_identifier_or_else(|| {})
                    .expect("Define without a legal name");

                match &d.body {
                    ExprKind::LambdaFunction(l) => {
                        let mut merge_defines = MergeDefines::new();
                        merge_defines.visit(&l.body);
                    }
                    _ => {
                        unimplemented!()
                    }
                }
            }
            _ => {}
        }
    }

    unimplemented!()
}

#[derive(PartialEq)]
enum ExpressionType<'a> {
    DefineFlat(&'a str),
    DefineFunction(&'a str),
    Expression,
}

fn convert_exprs_to_let(begin: Begin) -> ExprKind {
    // let defines = collect_defines_from_current_scope(&exprs);
    let mut expression_types = Vec::new();

    for expr in &begin.exprs {
        match expr {
            ExprKind::Define(d) => {
                let name = d
                    .name
                    .atom_identifier_or_else(|| {})
                    .expect("Define without a legal name");

                match &d.body {
                    ExprKind::LambdaFunction(_) => {
                        // let mut merge_defines = MergeDefines::new();
                        // merge_defines.visit(&l.body);
                        expression_types.push(ExpressionType::DefineFunction(name));
                    }
                    _ => {
                        expression_types.push(ExpressionType::DefineFlat(name));
                    }
                }
            }
            _ => expression_types.push(ExpressionType::Expression),
        }
    }

    // Go ahead and quit if there are
    if expression_types.iter().all(|x| {
        if let ExpressionType::Expression = x {
            true
        } else {
            false
        }
    }) {
        return ExprKind::Begin(begin);
    }

    let mut exprs = begin.exprs.clone();

    // let mut last_expression = expression_types.len();

    if let Some(idx) = expression_types.iter().rev().position(|x| {
        if let ExpressionType::Expression = x {
            false
        } else {
            true
        }
    }) {
        let idx = expression_types.len() - 1 - idx;

        // TODO
        let mut exprs = exprs.clone();

        let mut body = exprs.split_off(idx + 1);

        // These are going to be the
        let mut args = exprs
            .iter()
            .map(|x| {
                if let ExprKind::Define(d) = x {
                    d.body.clone()
                } else {
                    x.clone()
                }
            })
            .collect::<Vec<_>>();

        // This corresponds to the (let ((apple ..) (banana ..) (cucumber ..)))
        //                               ^^^^^^     ^^^^^^^      ^^^^^^^^
        let mut top_level_arguments: Vec<ExprKind> = Vec::new();
        // This corresponds to the set expressions
        // (set! apple #####apple0)
        // (set! banana #####banana1)
        // (set! cucumber #####cucumber1)
        let mut set_expressions: Vec<ExprKind> = Vec::new();

        // corresponds to #####apple0, #####banana1, #####cucumber1, etc
        let mut bound_names: Vec<ExprKind> = Vec::new();

        for (i, expression) in expression_types[0..idx + 1].into_iter().enumerate() {
            match expression {
                ExpressionType::DefineFunction(name) => {
                    if let ExprKind::Define(d) = &exprs[i] {
                        top_level_arguments.push(d.name.clone());
                        let name_prime = ExprKind::Atom(Atom::new(SyntaxObject::default(
                            TokenType::Identifier(
                                "#####".to_string() + name + i.to_string().as_str(),
                            ),
                        )));
                        let set_expr = ExprKind::Set(Box::new(Set::new(
                            d.name.clone(),
                            name_prime.clone(),
                            SyntaxObject::default(TokenType::Set),
                        )));
                        bound_names.push(name_prime);
                        set_expressions.push(set_expr);
                    } else {
                        panic!("expected define, found: {}", &exprs[i]);
                    };

                    // let name = Atom::new(SyntaxObject::new)
                }
                ExpressionType::DefineFlat(name) => {
                    if let ExprKind::Define(d) = &exprs[i] {
                        top_level_arguments.push(d.name.clone());
                        let name_prime = ExprKind::Atom(Atom::new(SyntaxObject::default(
                            TokenType::Identifier(
                                "#####".to_string() + name + i.to_string().as_str(),
                            ),
                        )));
                        let set_expr = ExprKind::Set(Box::new(Set::new(
                            d.name.clone(),
                            name_prime.clone(),
                            SyntaxObject::default(TokenType::Set),
                        )));
                        bound_names.push(name_prime);
                        set_expressions.push(set_expr);
                    } else {
                        panic!("expected define, found: {}", &exprs[i]);
                    };
                }
                ExpressionType::Expression => {
                    let expr =
                        ExprKind::Atom(Atom::new(SyntaxObject::default(TokenType::Identifier(
                            "#####define-conversion".to_string() + i.to_string().as_str(),
                        ))));

                    // This also gets bound in the inner function for now
                    bound_names.push(expr.clone());

                    top_level_arguments.push(expr);
                }
            }
        }

        // Top level application with dummy arguments that will immediately get overwritten
        let mut top_level_dummy_args = vec![
            ExprKind::Atom(Atom::new(SyntaxObject::default(
                TokenType::IntegerLiteral(123)
            )));
            top_level_arguments.len()
        ];

        // Append the body instructions to the set!
        set_expressions.append(&mut body);

        let inner_lambda = LambdaFunction::new(
            bound_names,
            ExprKind::Begin(Begin::new(
                set_expressions,
                SyntaxObject::default(TokenType::Begin),
            )),
            SyntaxObject::default(TokenType::Lambda),
        );

        args.insert(0, ExprKind::LambdaFunction(Box::new(inner_lambda)));

        let inner_application = ExprKind::List(List::new(args));

        let outer_lambda = LambdaFunction::new(
            top_level_arguments,
            inner_application,
            SyntaxObject::default(TokenType::Lambda),
        );

        top_level_dummy_args.insert(0, ExprKind::LambdaFunction(Box::new(outer_lambda)));

        return ExprKind::List(List::new(top_level_dummy_args));
    } else {
        panic!("Convert exprs to let in define conversion found no trailing expressions in begin")
    }

    // let mut lambda = LambdaFunction::new

    // unimplemented!()
}

#[cfg(test)]
mod flatten_begin_test {

    use super::*;
    use crate::parser::ast::ExprKind;
    use crate::parser::ast::{Atom, Begin, List};

    use crate::parser::parser::SyntaxObject;
    use crate::parser::tokens::TokenType;
    use crate::parser::tokens::TokenType::*;

    #[test]
    fn basic_flatten_one_level() {
        let expr = ExprKind::Begin(Begin::new(
            vec![
                ExprKind::Begin(Begin::new(
                    vec![ExprKind::List(List::new(vec![
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "+".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "x".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(10)))),
                    ]))],
                    SyntaxObject::default(TokenType::Begin),
                )),
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "y".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(20)))),
                ])),
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "z".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(30)))),
                ])),
            ],
            SyntaxObject::default(TokenType::Begin),
        ));

        let expected = ExprKind::Begin(Begin::new(
            vec![
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "x".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(10)))),
                ])),
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "y".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(20)))),
                ])),
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "z".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(30)))),
                ])),
            ],
            SyntaxObject::default(TokenType::Begin),
        ));

        assert_eq!(FlattenBegin::flatten(expr), expected);
    }

    #[test]
    fn basic_flatten_multiple_levels() {
        let expr = ExprKind::Begin(Begin::new(
            vec![
                ExprKind::Begin(Begin::new(
                    vec![ExprKind::List(List::new(vec![
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "+".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "x".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(10)))),
                    ]))],
                    SyntaxObject::default(TokenType::Begin),
                )),
                ExprKind::Begin(Begin::new(
                    vec![ExprKind::List(List::new(vec![
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "+".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "y".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(20)))),
                    ]))],
                    SyntaxObject::default(TokenType::Begin),
                )),
                ExprKind::Begin(Begin::new(
                    vec![ExprKind::List(List::new(vec![
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "+".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                            "z".to_string(),
                        )))),
                        ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(30)))),
                    ]))],
                    SyntaxObject::default(TokenType::Begin),
                )),
            ],
            SyntaxObject::default(TokenType::Begin),
        ));

        let expected = ExprKind::Begin(Begin::new(
            vec![
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "x".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(10)))),
                ])),
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "y".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(20)))),
                ])),
                ExprKind::List(List::new(vec![
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "+".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(Identifier(
                        "z".to_string(),
                    )))),
                    ExprKind::Atom(Atom::new(SyntaxObject::default(IntegerLiteral(30)))),
                ])),
            ],
            SyntaxObject::default(TokenType::Begin),
        ));

        assert_eq!(FlattenBegin::flatten(expr), expected);
    }
}