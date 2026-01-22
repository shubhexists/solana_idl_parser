use crate::config::{Config, GrpcConfig};
use anyhow::Result;
use futures::stream::StreamExt;
use rustls::crypto::ring;
use serde::Serialize;
use solana_account_decoder_client_types::token::UiTokenAmount;
use solana_sdk::{
    hash::Hash,
    message::{
        AccountMeta, Instruction, MessageHeader, VersionedMessage,
        compiled_instruction::CompiledInstruction,
        v0::{LoadedAddresses, Message, MessageAddressTableLookup},
    },
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use solana_transaction_context::TransactionReturnData;
use solana_transaction_status::{
    ConfirmedTransactionWithStatusMeta, InnerInstruction, InnerInstructions, Reward, RewardType,
    TransactionStatusMeta, TransactionTokenBalance, TransactionWithStatusMeta,
    VersionedTransactionWithStatusMeta,
};
use std::{
    collections::HashMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use yellowstone_grpc_client::{GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::{
    SubscribeRequest, SubscribeRequestFilterTransactions, subscribe_update::UpdateOneof,
};

mod config;
#[derive(Debug, Serialize)]
struct TransactionInstructionWithParent {
    instruction: Instruction,
    parent_program_id: Option<Pubkey>,
}

type TxnFilterMap = HashMap<String, SubscribeRequestFilterTransactions>;
pub const PUMP_FUN_AMM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

solana_idl_parser::parse_idl!("../idl/idl.json");

#[tokio::main]
async fn main() -> Result<()> {
    ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");
    let config = Config::read_from_file(Path::new("./config.toml"))?;

    start_grpc_processing(config.grpc).await?;
    Ok(())
}

async fn start_grpc_processing(grpc_config: GrpcConfig) -> Result<()> {
    let client = grpc_config.connect().await?;
    let request: SubscribeRequest = grpc_config.get_tx_updates()?;
    grpc_subscribe(client, request).await?;
    Ok(())
}

async fn grpc_subscribe(
    mut client: GeyserGrpcClient<impl Interceptor>,
    request: SubscribeRequest,
) -> Result<()> {
    let (_, mut stream) = client.subscribe_with_request(Some(request)).await?;
    while let Some(message) = stream.next().await {
        match message {
            Ok(msg) => match msg.update_oneof {
                Some(UpdateOneof::Transaction(update)) => {
                    let slot = update.slot;
                    let block_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("Time went backwards")
                        .as_secs() as i64;
                    let update: Option<
                        yellowstone_grpc_proto::prelude::SubscribeUpdateTransactionInfo,
                    > = update.transaction;

                    if let Some(txn) = update {
                        let raw_signature = txn.signature.clone();
                        let raw_transaction = txn.transaction.expect("transaction empty");
                        let raw_message = raw_transaction.message.expect("message empty").clone();
                        let header = raw_message.header.expect("header empty");
                        let meta = txn.meta.expect("Meta empty");

                        if raw_signature.len() != 64 {
                            panic!("Signature must be exactly 64 bytes");
                        }

                        let raw_signature_array: [u8; 64] = raw_signature
                            .try_into()
                            .expect("Failed to convert to [u8; 64]");
                        let signature = Signature::from(raw_signature_array);
                        let recent_blockhash = Hash::new_from_array(
                            raw_message
                                .recent_blockhash
                                .clone()
                                .try_into()
                                .expect("Failed to convert Vec<u8> to [u8; 32]"),
                        );

                        let confirmed_txn_with_meta: ConfirmedTransactionWithStatusMeta = ConfirmedTransactionWithStatusMeta {
                                slot,
                                tx_with_meta: TransactionWithStatusMeta::Complete(
                                    VersionedTransactionWithStatusMeta {
                                        transaction: VersionedTransaction {
                                            signatures: vec![signature],
                                            message: VersionedMessage::V0(Message {
                                                header: MessageHeader {
                                                    num_required_signatures: header.num_required_signatures as u8,
                                                    num_readonly_signed_accounts: header.num_readonly_signed_accounts as u8,
                                                    num_readonly_unsigned_accounts: header.num_readonly_unsigned_accounts as u8,
                                                },
                                                account_keys: raw_message.account_keys
                                                    .iter()
                                                    .map(|k: &Vec<u8>| {
                                                        k.clone()
                                                            .try_into()
                                                            .expect(
                                                                "Failed to convert Vec<u8> to [u8; 32]"
                                                            )
                                                    })
                                                    .collect(),
                                                recent_blockhash,
                                                instructions: raw_message.instructions
                                                    .iter()
                                                    .map(|ix| CompiledInstruction {
                                                        program_id_index: ix.program_id_index as u8,
                                                        accounts: ix.accounts.clone(),
                                                        data: ix.data.clone(),
                                                    })
                                                    .collect(),
                                                address_table_lookups: raw_message.address_table_lookups
                                                    .iter()
                                                    .map(|l| MessageAddressTableLookup {
                                                        account_key: Pubkey::new_from_array(
                                                            l.account_key
                                                                .clone()
                                                                .try_into()
                                                                .expect(
                                                                    "Failed to convert Vec<u8> to [u8; 32]"
                                                                )
                                                        ),
                                                        writable_indexes: l.writable_indexes.clone(),
                                                        readonly_indexes: l.readonly_indexes.clone(),
                                                    })
                                                    .collect(),
                                            }),
                                        },
                                        meta: TransactionStatusMeta {
                                            status: Ok(()),
                                            fee: meta.fee,
                                            cost_units: None,
                                            pre_balances: meta.pre_balances.clone(),
                                            post_balances: meta.post_balances.clone(),
                                            inner_instructions: Some(
                                                meta.inner_instructions
                                                    .iter()
                                                    .map(|f| {
                                                        InnerInstructions {
                                                            index: f.index as u8,
                                                            instructions: f.instructions
                                                                .iter()
                                                                .map(|v| {
                                                                    InnerInstruction {
                                                                        instruction: CompiledInstruction {
                                                                            program_id_index: v.program_id_index as u8,
                                                                            accounts: v.accounts.clone(),
                                                                            data: v.data.clone(),
                                                                        },
                                                                        stack_height: Some(
                                                                            v.stack_height.unwrap()
                                                                        ),
                                                                    }
                                                                })
                                                                .collect(),
                                                        }
                                                    })
                                                    .collect()
                                            ),
                                            log_messages: Some(
                                                meta.log_messages
                                                    .iter()
                                                    .map(|f| f.clone())
                                                    .collect::<Vec<String>>()
                                            ),
                                            pre_token_balances: Some(
                                                meta.pre_token_balances
                                                    .iter()
                                                    .map(|tb| TransactionTokenBalance {
                                                        account_index: tb.account_index as u8,
                                                        mint: tb.mint.clone(),
                                                        ui_token_amount: UiTokenAmount {
                                                            ui_amount: {
                                                                let ui_token_amount =
                                                                    tb.ui_token_amount
                                                                        .clone()
                                                                        .unwrap_or_default();
                                                                if ui_token_amount.ui_amount == 0.0 {
                                                                    None
                                                                } else {
                                                                    Some(ui_token_amount.ui_amount)
                                                                }
                                                            },
                                                            decimals: tb.ui_token_amount
                                                                .clone()
                                                                .unwrap_or_default().decimals as u8,
                                                            amount: tb.ui_token_amount
                                                                .clone()
                                                                .unwrap_or_default().amount,
                                                            ui_amount_string: tb.ui_token_amount
                                                                .clone()
                                                                .unwrap_or_default().ui_amount_string,
                                                        },

                                                        owner: tb.clone().owner,
                                                        program_id: tb.clone().program_id,
                                                    })
                                                    .collect()
                                            ),
                                            post_token_balances: Some(
                                                meta.post_token_balances
                                                    .iter()
                                                    .map(|tb| TransactionTokenBalance {
                                                        account_index: tb.account_index as u8,
                                                        mint: tb.mint.clone(),
                                                        ui_token_amount: UiTokenAmount {
                                                            ui_amount: {
                                                                let ui_token_amount =
                                                                    tb.ui_token_amount
                                                                        .clone()
                                                                        .unwrap_or_default();
                                                                if ui_token_amount.ui_amount == 0.0 {
                                                                    None
                                                                } else {
                                                                    Some(ui_token_amount.ui_amount)
                                                                }
                                                            },
                                                            decimals: tb.ui_token_amount
                                                                .clone()
                                                                .unwrap_or_default().decimals as u8,
                                                            amount: tb.ui_token_amount
                                                                .clone()
                                                                .unwrap_or_default().amount,
                                                            ui_amount_string: tb.ui_token_amount
                                                                .clone()
                                                                .unwrap_or_default().ui_amount_string,
                                                        },

                                                        owner: tb.clone().owner,
                                                        program_id: tb.clone().program_id,
                                                    })
                                                    .collect()
                                            ),
                                            rewards: Some(
                                                meta.rewards
                                                    .iter()
                                                    .map(|r| Reward {
                                                        pubkey: r.clone().pubkey,
                                                        lamports: r.lamports,
                                                        post_balance: r.post_balance,
                                                        reward_type: match r.reward_type {
                                                            0 => Some(RewardType::Fee),
                                                            1 => Some(RewardType::Rent),
                                                            2 => Some(RewardType::Staking),
                                                            3 => Some(RewardType::Voting),
                                                            _ => None,
                                                        },
                                                        commission: Some(unsafe {
                                                            r.clone().commission.as_bytes_mut()[0]
                                                        }),
                                                    })
                                                    .collect::<Vec<_>>()
                                            ),
                                            loaded_addresses: LoadedAddresses {
                                                writable: meta.loaded_writable_addresses
                                                    .iter()
                                                    .map(|addr|
                                                        Pubkey::new_from_array(
                                                            addr
                                                                .clone()
                                                                .try_into()
                                                                .expect(
                                                                    "Failed to convert Vec<u8> to [u8; 32]"
                                                                )
                                                        )
                                                    )
                                                    .collect(),
                                                readonly: meta.loaded_readonly_addresses
                                                    .iter()
                                                    .map(|addr|
                                                        Pubkey::new_from_array(
                                                            addr
                                                                .clone()
                                                                .try_into()
                                                                .expect(
                                                                    "Failed to convert Vec<u8> to [u8; 32]"
                                                                )
                                                        )
                                                    )
                                                    .collect(),
                                            },
                                            return_data: meta.return_data
                                                .as_ref()
                                                .map(|return_data| TransactionReturnData {
                                                    program_id: Pubkey::new_from_array(
                                                        return_data.program_id
                                                            .clone()
                                                            .try_into()
                                                            .expect(
                                                                "Failed to convert Vec<u8> to [u8; 32]"
                                                            )
                                                    ),
                                                    data: return_data.data.clone(),
                                                }),
                                            compute_units_consumed: Some(
                                                meta.compute_units_consumed.unwrap()
                                            ),
                                        },
                                    }
                                ),
                                block_time: Some(block_time),
                            };

                        let compiled_instructions: Vec<TransactionInstructionWithParent> =
                            match &confirmed_txn_with_meta.tx_with_meta {
                                TransactionWithStatusMeta::Complete(versioned_tx_with_meta) => {
                                    flatten_compiled_instructions(versioned_tx_with_meta)
                                }
                                TransactionWithStatusMeta::MissingMetadata(_) => {
                                    vec![]
                                }
                            };

                        let parsed_inner_instructions: Vec<TransactionInstructionWithParent> =
                            match &confirmed_txn_with_meta.tx_with_meta {
                                TransactionWithStatusMeta::Complete(versioned_tx_with_meta) => {
                                    flatten_inner_instructions(versioned_tx_with_meta)
                                }
                                TransactionWithStatusMeta::MissingMetadata(_) => {
                                    vec![]
                                }
                            };

                        compiled_instructions.iter().for_each(|instruction| {
                            let accounts = &instruction.instruction.accounts;
                            match PumpAmmInstructions::deserialize(
                                accounts.to_vec(),
                                &instruction.instruction.data,
                            ) {
                                Ok(decoded_ix) => match decoded_ix {
                                    PumpAmmInstructions::AdminSetCoinCreator(
                                        accounts,
                                        admin_set_coin_creator_args,
                                    ) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", admin_set_coin_creator_args);
                                    }
                                    PumpAmmInstructions::AdminUpdateTokenIncentives(
                                        accounts,
                                        admin_update_token_incentives_args,
                                    ) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", admin_update_token_incentives_args);
                                    }
                                    PumpAmmInstructions::Buy(accounts, buy_args) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", buy_args);
                                    }
                                    PumpAmmInstructions::BuyExactQuoteIn(
                                        accounts,
                                        buy_exact_quote_in_args,
                                    ) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", buy_exact_quote_in_args);
                                    }
                                    PumpAmmInstructions::Sell(accounts, sell_args) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", sell_args);
                                    }
                                    _ => {}
                                },
                                Err(_) => {}
                            }
                        });
                        parsed_inner_instructions.iter().for_each(|instruction| {
                            let accounts = &instruction.instruction.accounts;
                            match PumpAmmInstructions::deserialize(
                                accounts.to_vec(),
                                &instruction.instruction.data,
                            ) {
                                Ok(decoded_ix) => match decoded_ix {
                                    PumpAmmInstructions::AdminSetCoinCreator(
                                        accounts,
                                        admin_set_coin_creator_args,
                                    ) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", admin_set_coin_creator_args);
                                    }
                                    PumpAmmInstructions::AdminUpdateTokenIncentives(
                                        accounts,
                                        admin_update_token_incentives_args,
                                    ) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", admin_update_token_incentives_args);
                                    }
                                    PumpAmmInstructions::Buy(accounts, buy_args) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", buy_args);
                                    }
                                    PumpAmmInstructions::BuyExactQuoteIn(
                                        accounts,
                                        buy_exact_quote_in_args,
                                    ) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", buy_exact_quote_in_args);
                                    }
                                    PumpAmmInstructions::Sell(accounts, sell_args) => {
                                        println!("{:?}", accounts);
                                        println!("{:?}", sell_args);
                                    }
                                    _ => {}
                                },
                                Err(_) => {}
                            }
                        });
                    }
                }
                None => {}
                _ => {}
            },
            Err(_) => {
                break;
            }
        }
    }

    Ok(())
}

fn flatten_compiled_instructions(
    transaction_with_meta: &VersionedTransactionWithStatusMeta,
) -> Vec<TransactionInstructionWithParent> {
    let mut compiled_result = Vec::new();
    let transaction = &transaction_with_meta.transaction;
    let ci_ixs = transaction.message.instructions();
    let parsed_accounts = parse_transaction_accounts(
        &transaction.message,
        transaction_with_meta.meta.loaded_addresses.clone(),
    );

    for ci_ix in ci_ixs {
        compiled_result.push(TransactionInstructionWithParent {
            instruction: compiled_instruction_to_instruction(&ci_ix, parsed_accounts.clone()),
            parent_program_id: None,
        });
    }

    compiled_result
}

fn flatten_inner_instructions(
    transaction_with_meta: &VersionedTransactionWithStatusMeta,
) -> Vec<TransactionInstructionWithParent> {
    let mut inner_result = Vec::new();
    let transaction = &transaction_with_meta.transaction;
    let ci_ixs = transaction.message.instructions();
    let parsed_accounts = parse_transaction_accounts(
        &transaction.message,
        transaction_with_meta.meta.loaded_addresses.clone(),
    );

    if let Some(inner_ixs) = &transaction_with_meta.meta.inner_instructions {
        let mut ordered_cii = inner_ixs.clone();
        ordered_cii.sort_by(|a, b| a.index.cmp(&b.index));

        for cii in ordered_cii {
            let parent_program_id =
                parsed_accounts[ci_ixs[cii.index as usize].program_id_index as usize].pubkey;

            for cii_entry in cii.instructions {
                let ix = CompiledInstruction {
                    program_id_index: cii_entry.instruction.program_id_index,
                    accounts: cii_entry.instruction.accounts.clone(),
                    data: cii_entry.instruction.data.clone(),
                };
                inner_result.push(TransactionInstructionWithParent {
                    instruction: compiled_instruction_to_instruction(&ix, parsed_accounts.clone()),
                    parent_program_id: Some(parent_program_id),
                });
            }
        }
    }

    inner_result
}

fn compiled_instruction_to_instruction(
    ci: &CompiledInstruction,
    parsed_accounts: Vec<AccountMeta>,
) -> Instruction {
    let program_id = parsed_accounts[ci.program_id_index as usize].pubkey;
    let accounts: Vec<AccountMeta> = ci.accounts
        .iter()
        .map(|&index| {
            if (index as usize) >= parsed_accounts.len() {
                panic!(
                    "Trying to resolve account at index {} while parsedAccounts is only {}. \
                Looks like you're trying to parse versioned transaction, make sure that LoadedAddresses are passed to the \
                parseTransactionAccounts function",
                    index,
                    parsed_accounts.len()
                );
            }
            parsed_accounts[index as usize].clone()
        })
        .collect();

    Instruction {
        program_id,
        accounts,
        data: ci.data.clone(),
    }
}

pub fn parse_transaction_accounts(
    message: &VersionedMessage,
    loaded_addresses: LoadedAddresses,
) -> Vec<AccountMeta> {
    let accounts = message.static_account_keys();
    let readonly_signed_accounts_count = message.header().num_readonly_signed_accounts as usize;
    let readonly_unsigned_accounts_count = message.header().num_readonly_unsigned_accounts as usize;
    let required_signatures_accounts_count = message.header().num_required_signatures as usize;
    let total_accounts = accounts.len();

    let mut parsed_accounts: Vec<AccountMeta> = accounts
        .iter()
        .enumerate()
        .map(|(index, pubkey)| {
            let is_writable = index
                < required_signatures_accounts_count - readonly_signed_accounts_count
                || (index >= required_signatures_accounts_count
                    && index < total_accounts - readonly_unsigned_accounts_count);

            AccountMeta {
                pubkey: *pubkey,
                is_signer: index < required_signatures_accounts_count,
                is_writable,
            }
        })
        .collect();

    parsed_accounts.extend(
        loaded_addresses
            .writable
            .into_iter()
            .map(|pubkey| AccountMeta {
                pubkey,
                is_signer: false,
                is_writable: true,
            }),
    );

    parsed_accounts.extend(
        loaded_addresses
            .readonly
            .into_iter()
            .map(|pubkey| AccountMeta {
                pubkey,
                is_signer: false,
                is_writable: false,
            }),
    );

    parsed_accounts
}
