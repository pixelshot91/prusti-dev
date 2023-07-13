use bencher::Bencher;
use viper::*;

bencher::benchmark_main!(ast_factory);
bencher::benchmark_group!(ast_factory, bench_build_program, bench_verify_program);

fn bench_build_program(bench: &mut Bencher) {
    bench.iter(|| build_program().execute().unwrap());
}

fn bench_verify_program(bench: &mut Bencher) {
    let backend = VerificationBackend::Silicon;
    let mut verifier = Verifier::new(backend);
    let program = build_program();

    bench.iter(move || verifier.verify(program));
}

fn build_program() -> Program {
    let wrap_domain_function = ast::domain_func(
        "wrap",
        &[ast::local_var_decl("x", ast::type_var("T"))],
        ast::domain_type(
            "Wrapper",
            &[(ast::type_var("T"), ast::type_var("T"))],
            &[ast::type_var("T")],
        ),
        false,
        "Wrapper",
    );

    let unwrap_domain_function = ast::domain_func(
        "unwrap",
        &[ast::local_var_decl(
            "x",
            ast::domain_type(
                "Wrapper",
                &[(ast::type_var("T"), ast::type_var("T"))],
                &[ast::type_var("T")],
            ),
        )],
        ast::type_var("T"),
        false,
        "Wrapper",
    );

    let wrapper_domain = ast::domain(
        "Wrapper",
        &[wrap_domain_function, unwrap_domain_function],
        &[ast::named_domain_axiom(
            "unwrap_wrapper",
            // forall x: T :: unwrap(wrap(x)) == x
            ast::forall(
                &[ast::local_var_decl("x", ast::type_var("T"))],
                &[],
                ast::eq_cmp(
                    ast::domain_func_app(
                        unwrap_domain_function,
                        &[ast::domain_func_app(
                            wrap_domain_function,
                            &[ast::local_var("x", ast::type_var("T"))],
                            &[(ast::type_var("T"), ast::type_var("T"))],
                        )],
                        &[(ast::type_var("T"), ast::type_var("T"))],
                    ),
                    ast::local_var("x", ast::type_var("T")),
                ),
            ),
            "Wrapper",
        )],
        &[ast::type_var("T")],
    );

    let value_field = ast::field("value", ast::int_type());

    let even_function = ast::function(
        "even",
        &[ast::local_var_decl("v", ast::int_type())],
        ast::bool_type(),
        &[],
        &[],
        ast::no_position(),
        Some(
            // x % 2 == 0
            ast::eq_cmp(
                ast::module(ast::local_var("v", ast::int_type()), ast::int_lit(2)),
                ast::int_lit(0),
            ),
        ),
    );

    let even_num_box_predicate = ast::predicate(
        "EvenNumBox",
        &[ast::local_var_decl("box", ast::ref_type())],
        Some(
            // acc(box.value) && even(box.value)
            ast::and(
                ast::field_access_predicate(
                    ast::field_access(
                        ast::local_var("box", ast::ref_type()),
                        ast::field("value", ast::int_type()),
                    ),
                    ast::full_perm(),
                ),
                ast::func_app(
                    "even",
                    &[ast::field_access(
                        ast::local_var("box", ast::ref_type()),
                        ast::field("value", ast::int_type()),
                    )],
                    ast::bool_type(),
                    ast::no_position(),
                ),
            ),
        ),
    );

    let method = ast::method(
        "build_even_num_box",
        &[ast::local_var_decl("v", ast::int_type())],
        &[ast::local_var_decl("box", ast::ref_type())],
        &[
            // even(v)
            ast::func_app(
                "even",
                &[ast::local_var("v", ast::int_type())],
                ast::bool_type(),
                ast::no_position(),
            ),
        ],
        &[
            // EvenNumBox(box)
            ast::predicate_access_predicate(
                ast::predicate_access(&[ast::local_var("box", ast::ref_type())], "EvenNumBox"),
                ast::full_perm(),
            ),
        ],
        Some(ast::seqn(
            &[
                // box = new(value)
                ast::new_stmt(
                    ast::local_var("box", ast::ref_type()),
                    &[ast::field("value", ast::int_type())],
                ),
                // box.value = unwrap(wrap(v))
                ast::field_assign(
                    ast::field_access(
                        ast::local_var("box", ast::ref_type()),
                        ast::field("value", ast::int_type()),
                    ),
                    ast::domain_func_app(
                        unwrap_domain_function,
                        &[ast::domain_func_app(
                            wrap_domain_function,
                            &[ast::local_var("v", ast::int_type())],
                            &[(ast::type_var("T"), ast::int_type())],
                        )],
                        &[(ast::type_var("T"), ast::int_type())],
                    ),
                ),
                // fold EvenNumBox(box)
                ast::fold(ast::predicate_access_predicate(
                    ast::predicate_access(&[ast::local_var("box", ast::ref_type())], "EvenNumBox"),
                    ast::full_perm(),
                )),
            ],
            &[],
        )),
    );

    ast::program(
        &[wrapper_domain],
        &[value_field],
        &[even_function],
        &[even_num_box_predicate],
        &[method],
    )
}
