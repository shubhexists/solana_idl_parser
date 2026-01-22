use crate::{TxnFilterMap, PUMP_FUN_AMM};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path, time::Duration};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions,
};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub grpc: GrpcConfig,
}

#[derive(Serialize, Deserialize)]
pub struct GrpcConfig {
    pub grpc_url: String,
    pub x_token: String,
}

impl Config {
    pub fn read_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let parsed_config = toml::from_str::<Config>(&content)?;
        Ok(parsed_config)
    }
}

impl GrpcConfig {
    pub async fn connect(&self) -> Result<GeyserGrpcClient<impl Interceptor>> {
        GeyserGrpcClient::build_from_shared(self.grpc_url.clone())?
            .x_token(Some(self.x_token.clone()))?
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(10))
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .max_decoding_message_size(1024 * 1024 * 1024)
            .connect()
            .await
            .map_err(Into::into)
    }

    pub fn get_tx_updates(&self) -> Result<SubscribeRequest> {
        let mut transactions: TxnFilterMap = HashMap::new();

        transactions.insert(
            "amm_contract_address".to_owned(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                account_include: vec![PUMP_FUN_AMM.to_string()],
                account_exclude: vec![],
                account_required: vec![],
                signature: None,
            },
        );

        Ok(SubscribeRequest {
            accounts: HashMap::default(),
            slots: HashMap::default(),
            transactions,
            transactions_status: HashMap::default(),
            blocks: HashMap::default(),
            blocks_meta: HashMap::default(),
            entry: HashMap::default(),
            commitment: Some(CommitmentLevel::Processed as i32),
            accounts_data_slice: Vec::default(),
            ping: None,
            from_slot: None,
        })
    }
}
