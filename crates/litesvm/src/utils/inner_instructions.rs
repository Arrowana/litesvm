use {
    solana_instruction::TRANSACTION_LEVEL_STACK_HEIGHT,
    solana_message::{
        compiled_instruction::CompiledInstruction,
        inner_instruction::{InnerInstruction, InnerInstructionsList},
    },
    solana_transaction_context::transaction::{ExecutionRecord, TransactionContext},
};

/// Adapted from the current `solana-svm` transaction deconstruction path.
pub fn deconstruct_transaction(
    mut transaction_context: TransactionContext,
) -> (ExecutionRecord, InnerInstructionsList) {
    debug_assert!(transaction_context
        .get_instruction_context_at_index_in_trace(0)
        .map(|instruction_context| instruction_context.get_stack_height()
            == TRANSACTION_LEVEL_STACK_HEIGHT)
        .unwrap_or(true));

    let top_level_ixs_num = transaction_context
        .get_instruction_trace_length()
        .saturating_sub(transaction_context.number_of_cpis_in_trace());
    let mut parent_positions = vec![usize::MAX; transaction_context.number_of_cpis_in_trace()];
    let (ix_trace, accounts, ix_data_trace) = transaction_context.take_instruction_trace();
    let mut outer_instructions: Vec<Vec<InnerInstruction>> = vec![Vec::new(); top_level_ixs_num];

    for (cpi_num, ((ix_in_trace, ix_data), ix_accounts)) in ix_trace
        .into_iter()
        .zip(ix_data_trace.into_iter())
        .zip(accounts)
        .skip(top_level_ixs_num)
        .enumerate()
    {
        let caller_ix = ix_in_trace.index_of_caller_instruction;
        debug_assert_ne!(caller_ix, u16::MAX, "Instruction is not a CPI");

        let outer_index = if (caller_ix as usize) < top_level_ixs_num {
            *parent_positions.get_mut(cpi_num).unwrap() = caller_ix as usize;
            caller_ix as usize
        } else {
            let caller_cpi_index = (caller_ix as usize).saturating_sub(top_level_ixs_num);
            if let Some(caller_index) = parent_positions.get(caller_cpi_index).copied() {
                if caller_index != usize::MAX {
                    *parent_positions.get_mut(cpi_num).unwrap() = caller_index;
                    caller_index
                } else {
                    debug_assert!(false);
                    usize::MAX
                }
            } else {
                debug_assert!(false);
                usize::MAX
            }
        };

        if let Some(inner_instructions) = outer_instructions.get_mut(outer_index) {
            let stack_height = ix_in_trace.nesting_level.saturating_add(1);
            let stack_height = u8::try_from(stack_height).unwrap_or(u8::MAX);
            inner_instructions.push(InnerInstruction {
                instruction: CompiledInstruction::new_from_raw_parts(
                    ix_in_trace.program_account_index_in_tx as u8,
                    ix_data.into_owned(),
                    ix_accounts
                        .iter()
                        .map(|account| account.index_in_transaction as u8)
                        .collect(),
                ),
                stack_height,
            });
        } else {
            debug_assert!(false);
        }
    }

    let record: ExecutionRecord = transaction_context.into();
    (record, outer_instructions)
}
