use color_eyre::Result;
use melior_next::ir::{Block, OperationRef, Region, Value};

use crate::compiler::fn_attributes::FnAttributes;
use crate::{
    compiler::{Compiler, Storage},
    sierra_type::SierraType,
    utility::create_fn_signature,
};

impl<'ctx> Compiler<'ctx> {
    fn create_enum_get_tag(
        &'ctx self,
        enum_type: &SierraType,
        storage: &mut Storage<'ctx>,
    ) -> Result<String> {
        let (enum_mlir_type, tag_type) = if let SierraType::Enum {
            ty,
            tag_type,
            storage_bytes_len: _,
            storage_type: _,
            variants_types: _,
        } = enum_type
        {
            (*ty, *tag_type)
        } else {
            panic!("create_enum_get_tag should have been passed an Enum SierraType, but was instead passed {:?}", enum_type)
        };

        let func_name = format!("enum_get_tag<{}>", enum_mlir_type);

        if storage.helperfuncs.contains(&func_name) {
            return Ok(func_name);
        }

        let region = Region::new();

        let block = region.append_block(Block::new(&[enum_type.get_type_location(&self.context)]));

        let enum_mlir_type = enum_type.get_type();
        let enum_value = block.argument(0)?.into();

        let enum_tag_op = self.op_llvm_extractvalue(&block, 0, enum_value, tag_type)?;
        let enum_tag = enum_tag_op.result(0)?.into();

        self.op_return(&block, &[enum_tag]);

        let function_type = create_fn_signature(&[enum_mlir_type], &[tag_type]);

        let func = self.op_func(
            &func_name,
            &function_type,
            vec![region],
            FnAttributes::libfunc(false, true),
        )?;

        storage.helperfuncs.insert(func_name.clone());

        self.module.body().append_operation(func);

        Ok(func_name)
    }

    fn create_enum_get_data_as_variant_type(
        &'ctx self,
        enum_type: &SierraType,
        variant: usize,
        storage: &mut Storage<'ctx>,
    ) -> Result<String> {
        let (enum_mlir_type, storage_type, variant_type) = if let SierraType::Enum {
            ty,
            tag_type: _,
            storage_bytes_len: _,
            storage_type,
            variants_types,
        } = enum_type
        {
            (*ty, *storage_type, variants_types[variant].get_type())
        } else {
            panic!("create_enum_get_data_as_variant_type should have been passed an Enum SierraType, but was instead passed {:?}", enum_type)
        };

        let func_name = format!("enum_get_data_as_variant_type<{}, {}>", enum_mlir_type, variant);

        if storage.helperfuncs.contains(&func_name) {
            return Ok(func_name);
        }

        let region = Region::new();

        let block = region.append_block(Block::new(&[enum_type.get_type_location(&self.context)]));

        let enum_mlir_type = enum_type.get_type();
        let enum_value = block.argument(0)?.into();

        let enum_data_op = self.op_llvm_extractvalue(&block, 1, enum_value, storage_type)?;
        let enum_data = enum_data_op.result(0)?.into();
        // Allocate space on the stack to store the enum_data in order to reinterpret it as the given type
        let data_ptr_op = self.op_llvm_alloca(&block, storage_type, 1)?;
        let data_ptr = data_ptr_op.result(0)?.into();
        // Store the data into the allocated space
        self.op_llvm_store(&block, enum_data, data_ptr)?;
        // Now read it back as the other type
        let cast_data_op = self.op_llvm_load(&block, data_ptr, variant_type)?;
        let cast_data = cast_data_op.result(0)?.into();

        self.op_return(&block, &[cast_data]);

        let function_type = create_fn_signature(&[enum_mlir_type], &[variant_type]);

        let func = self.op_func(
            &func_name,
            &function_type,
            vec![region],
            FnAttributes::libfunc(false, true),
        )?;

        storage.helperfuncs.insert(func_name.clone());

        self.module.body().append_operation(func);

        Ok(func_name)
    }
}

impl<'ctx> Compiler<'ctx> {
    pub fn call_enum_get_tag<'block>(
        &'ctx self,
        block: &'block Block,
        enum_value: Value,
        enum_type: &SierraType,
        storage: &mut Storage<'ctx>,
    ) -> Result<OperationRef<'block>> {
        let tag_type = if let SierraType::Enum {
            ty: _,
            tag_type,
            storage_bytes_len: _,
            storage_type: _,
            variants_types: _,
        } = enum_type
        {
            *tag_type
        } else {
            panic!("call_enum_get_tag should have been passed an enum type, but instead was passed {:?}", enum_type)
        };
        let func_name = self.create_enum_get_tag(enum_type, storage)?;
        self.op_llvm_call(block, &func_name, &[enum_value], &[tag_type])
    }

    pub fn call_enum_get_data_as_variant_type<'block>(
        &'ctx self,
        block: &'block Block,
        enum_value: Value,
        enum_type: &SierraType,
        variant: usize,
        storage: &mut Storage<'ctx>,
    ) -> Result<OperationRef<'block>> {
        let variant_type = if let SierraType::Enum {
            ty: _,
            tag_type: _,
            storage_bytes_len: _,
            storage_type: _,
            variants_types,
        } = enum_type
        {
            variants_types[variant].get_type()
        } else {
            panic!("call_enum_get_data_as_variant_type should have been passed an enum type, but instead was passed {:?}", enum_type)
        };
        let func_name = self.create_enum_get_data_as_variant_type(enum_type, variant, storage)?;
        self.op_llvm_call(block, &func_name, &[enum_value], &[variant_type])
    }
}
