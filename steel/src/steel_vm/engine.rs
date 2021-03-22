use std::{
    collections::HashMap,
    convert::TryFrom,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
};

use super::{evaluation_progress::Callback, primitives::embed_primitives, vm::VirtualMachineCore};
use crate::{
    compiler::{compiler::Compiler, constants::ConstantMap, program::Program},
    core::instructions::DenseInstruction,
    parser::ast::ExprKind,
    parser::parser::{ParseError, Parser},
    primitives::ListOperations,
    rerrs::{ErrorKind, SteelErr},
    rvals::{FromSteelVal, IntoSteelVal, Result, SteelVal},
    stop, throw,
};

pub struct Engine {
    virtual_machine: VirtualMachineCore,
    compiler: Compiler,
}

impl Engine {
    /// Instantiates a raw engine instance. Includes no primitives or prelude.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_raw();
    /// assert!(vm.run("(+ 1 2 3").is_err()); // + is a free identifier
    /// ```
    pub fn new_raw() -> Self {
        Engine {
            virtual_machine: VirtualMachineCore::new(),
            compiler: Compiler::default(),
        }
    }

    /// Instantiates a new engine instance with all primitive functions enabled.
    /// This excludes the prelude and contract files.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_base();
    /// // map is found in the prelude, so this will fail
    /// assert!(vm.run(r#"(map (lambda (x) 10) (list 1 2 3 4 5))"#).is_err());
    /// ```
    #[inline]
    pub fn new_base() -> Self {
        let mut vm = Engine::new_raw();
        // Embed any primitives that we want to use
        embed_primitives(&mut vm);
        vm
    }

    /// Instantiates a new engine instance with all the primitive functions enabled.
    /// This is the most general engine entry point, and includes both the contract and
    /// prelude files in the root.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new();
    /// vm.run(r#"(+ 1 2 3)"#).unwrap();
    /// ```
    pub fn new() -> Self {
        let mut vm = Engine::new_base();

        let core_libraries = &[crate::stdlib::PRELUDE, crate::stdlib::CONTRACTS];

        for core in core_libraries {
            vm.parse_and_execute_without_optimizations(core).unwrap();
        }

        vm
    }

    pub fn new_with_meta() -> Engine {
        let mut vm = Engine::new();
        vm.register_value("*env*", crate::env::Env::constant_env_to_hashmap());
        vm
    }

    /// Consumes the current `Engine` and emits a new `Engine` with the prelude added
    /// to the environment. The prelude won't work unless the primitives are also enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_base().with_prelude().unwrap();
    /// vm.run("(+ 1 2 3)").unwrap();
    /// ```
    pub fn with_prelude(mut self) -> Result<Self> {
        let core_libraries = &[crate::stdlib::PRELUDE, crate::stdlib::CONTRACTS];

        for core in core_libraries {
            self.parse_and_execute_without_optimizations(core)?;
        }

        Ok(self)
    }

    /// Registers the prelude to the environment of the given Engine.
    /// The prelude won't work unless the primitives are also enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// let mut vm = Engine::new_base();
    /// vm.register_prelude().unwrap();
    /// vm.run("(+ 1 2 3)").unwrap();
    /// ```
    pub fn register_prelude(&mut self) -> Result<&mut Self> {
        let core_libraries = &[crate::stdlib::PRELUDE, crate::stdlib::CONTRACTS];

        for core in core_libraries {
            self.parse_and_execute_without_optimizations(core)?;
        }

        Ok(self)
    }

    pub fn emit_program_with_path(&mut self, expr: &str, path: PathBuf) -> Result<Program> {
        self.compiler.compile_program(expr, Some(path))
    }

    pub fn emit_program(&mut self, expr: &str) -> Result<Program> {
        self.compiler.compile_program(expr, None)
    }

    pub fn execute(
        &mut self,
        bytecode: Rc<[DenseInstruction]>,
        constant_map: &ConstantMap,
    ) -> Result<SteelVal> {
        self.virtual_machine.execute(bytecode, constant_map)
    }

    pub fn emit_instructions_with_path(
        &mut self,
        exprs: &str,
        path: PathBuf,
    ) -> Result<Vec<Vec<DenseInstruction>>> {
        self.compiler.emit_instructions(exprs, Some(path))
    }

    pub fn emit_instructions(&mut self, exprs: &str) -> Result<Vec<Vec<DenseInstruction>>> {
        self.compiler.emit_instructions(exprs, None)
    }

    pub fn execute_program(&mut self, program: Program) -> Result<Vec<SteelVal>> {
        self.virtual_machine.execute_program(program)
    }

    pub fn register_external_value<T: FromSteelVal + IntoSteelVal>(
        &mut self,
        name: &str,
        value: T,
    ) -> Result<&mut Self> {
        let converted = value.into_steelval()?;
        Ok(self.register_value(name, converted))
    }

    pub fn register_value(&mut self, name: &str, value: SteelVal) -> &mut Self {
        let idx = self.compiler.register(name);
        self.virtual_machine.insert_binding(idx, value);
        self
    }

    pub fn register_values(
        &mut self,
        values: impl Iterator<Item = (String, SteelVal)>,
    ) -> &mut Self {
        for (name, value) in values {
            self.register_value(name.as_str(), value);
        }
        self
    }

    /// Registers a predicate for a given type. When embedding external values, it is convenient
    /// to be able to have a predicate to test if the given value is the specified type.
    /// In order to be registered, a type must implement [`FromSteelVal`](crate::rvals::FromSteelVal)
    /// and [`IntoSteelVal`](crate::rvals::IntoSteelVal)
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate steel;
    /// # use steel::steel_vm::engine::Engine;
    /// use steel::steel_vm::register_fn::RegisterFn;
    /// fn foo() -> usize {
    ///    10
    /// }
    ///
    /// let mut vm = Engine::new();
    /// vm.register_fn("foo", foo);
    ///
    /// vm.run(r#"(foo)"#).unwrap(); // Returns vec![10]
    /// ```
    pub fn register_type<T: FromSteelVal + IntoSteelVal>(
        &mut self,
        predicate_name: &'static str,
    ) -> &mut Self {
        let f = move |args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => format!("{} expected 1 argument, got {}", predicate_name, args.len()));
            }

            Ok(SteelVal::BoolV(T::from_steelval(args[0].clone()).is_ok()))
        };

        self.register_value(predicate_name, SteelVal::BoxedFunction(Rc::new(f)))
    }

    /// Registers a callback function
    /// If registered, this callback will be called on every instruction
    pub fn on_progress(&mut self, callback: Callback) -> &mut Self {
        self.virtual_machine.on_progress(callback);
        self
    }

    pub fn extract_value(&self, name: &str) -> Result<SteelVal> {
        let idx = self.compiler.get_idx(name).ok_or_else(throw!(
            Generic => format!("free identifier: {} - identifier given cannot be found in the global environment", name)
        ))?;

        self.virtual_machine.extract_value(idx)
            .ok_or_else(throw!(
                Generic => format!("free identifier: {} - identifier given cannot be found in the global environment", name)
            ))
    }

    pub fn extract<T: FromSteelVal>(&self, name: &str) -> Result<T> {
        T::from_steelval(self.extract_value(name)?)
    }

    pub fn run(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
        let program = self.compiler.compile_program(expr, None)?;
        self.virtual_machine.execute_program(program)
    }

    pub fn run_with_path(&mut self, expr: &str, path: PathBuf) -> Result<Vec<SteelVal>> {
        let program = self.compiler.compile_program(expr, Some(path))?;
        self.virtual_machine.execute_program(program)
    }

    pub fn parse_and_execute_without_optimizations(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
        let program = self.compiler.compile_program(expr, None)?;
        self.virtual_machine.execute_program(program)
    }

    pub fn parse_and_execute(&mut self, expr: &str) -> Result<Vec<SteelVal>> {
        self.parse_and_execute_without_optimizations(expr)
    }

    // Read in the file from the given path and execute accordingly
    // Loads all the functions in from the given env
    // pub fn parse_and_execute_from_path<P: AsRef<Path>>(
    //     &mut self,
    //     path: P,
    // ) -> Result<Vec<SteelVal>> {
    //     let mut file = std::fs::File::open(path)?;
    //     let mut exprs = String::new();
    //     file.read_to_string(&mut exprs)?;
    //     self.parse_and_execute(exprs.as_str(), )
    // }

    pub fn parse_and_execute_from_path<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<Vec<SteelVal>> {
        let path_buf = PathBuf::from(path.as_ref());
        let mut file = std::fs::File::open(path)?;
        let mut exprs = String::new();
        file.read_to_string(&mut exprs)?;
        self.run_with_path(exprs.as_str(), path_buf)
    }

    // TODO come back to this please

    pub fn parse_and_execute_with_optimizations(
        &mut self,
        expr: &str,
        path: PathBuf,
    ) -> Result<Vec<SteelVal>> {
        let mut results = Vec::new();
        let mut intern = HashMap::new();

        let parsed: std::result::Result<Vec<ExprKind>, ParseError> =
            Parser::new_from_source(expr, &mut intern, path.clone()).collect();
        let parsed = parsed?;

        let expanded_statements = self
            .compiler
            .expand_expressions(parsed, Some(path.clone()))?;

        let statements_without_structs = self
            .compiler
            .extract_structs(expanded_statements, &mut results)?;

        let exprs_post_optimization = Self::optimize_exprs(statements_without_structs)?;

        let compiled_instructions = self
            .compiler
            .generate_dense_instructions(exprs_post_optimization, results)?;

        let program = Program::new(
            compiled_instructions,
            (&self.compiler.constant_map).to_bytes()?,
        );

        self.virtual_machine.execute_program(program)
    }

    // TODO come back to this
    pub fn optimize_exprs<I: IntoIterator<Item = ExprKind>>(exprs: I) -> Result<Vec<ExprKind>> {
        // println!("About to optimize the input program");

        let converted: Result<Vec<_>> = exprs.into_iter().map(|x| SteelVal::try_from(x)).collect();

        // let converted = Gc::new(SteelVal::try_from(v[0].clone())?);
        let exprs = ListOperations::built_in_list_func_flat_non_gc(converted?)?;

        let mut vm = Engine::new_with_meta();
        vm.parse_and_execute_without_optimizations(crate::stdlib::PRELUDE)?;
        vm.register_value("*program*", exprs);
        let output = vm.parse_and_execute_without_optimizations(crate::stdlib::COMPILER)?;

        // println!("{:?}", output.last().unwrap());

        // if output.len()  1 {
        //     stop!(Generic => "panic! internal compiler error: output did not return a valid program");
        // }

        // TODO
        SteelVal::iter(output.last().unwrap().clone())
            .into_iter()
            .map(|x| {
                ExprKind::try_from(&x).map_err(|x| SteelErr::new(ErrorKind::Generic, x.to_string()))
            })
            .collect::<Result<Vec<ExprKind>>>()
    }
}
