use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Idl {
    pub address: String,
    pub metadata: IdlMetadata,
    pub instructions: Vec<IdlInstruction>,
    #[serde(default)]
    pub accounts: Vec<IdlAccount>,
    #[serde(default)]
    pub types: Vec<IdlTypeDef>,
    #[serde(default)]
    pub events: Vec<IdlEvent>,
    #[serde(default)]
    pub errors: Vec<IdlError>,
}

#[derive(Debug, Deserialize)]
pub struct IdlMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub spec: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IdlInstruction {
    pub name: String,
    #[serde(default)]
    pub docs: Vec<String>,
    pub discriminator: Vec<u8>,
    pub accounts: Vec<IdlInstructionAccount>,
    #[serde(default)]
    pub args: Vec<IdlField>,
}

#[derive(Debug, Deserialize)]
pub struct IdlInstructionAccount {
    pub name: String,
    #[serde(default)]
    pub writable: bool,
    #[serde(default)]
    pub signer: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub pda: Option<IdlPda>,
    #[serde(default)]
    pub relations: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct IdlPda {
    pub seeds: Vec<IdlSeed>,
    #[serde(default)]
    pub program: Option<IdlSeed>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "lowercase")]
pub enum IdlSeed {
    Const {
        value: Vec<u8>,
    },
    Arg {
        path: String,
    },
    Account {
        path: String,
        #[serde(default)]
        account: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
pub struct IdlField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlType,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum IdlType {
    Primitive(String),
    Defined { defined: IdlDefinedType },
    Option { option: Box<IdlType> },
    Vec { vec: Box<IdlType> },
    Array { array: (Box<IdlType>, usize) },
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum IdlDefinedType {
    Simple(String),
    Named { name: String },
}

#[derive(Debug, Deserialize)]
pub struct IdlAccount {
    pub name: String,
    pub discriminator: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct IdlTypeDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlTypeDefType,
}

#[derive(Debug, Deserialize)]
pub struct IdlTypeDefType {
    pub kind: String,
    #[serde(default)]
    pub fields: IdlTypeDefFields,
    #[serde(default)]
    pub variants: Vec<IdlEnumVariant>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
pub enum IdlTypeDefFields {
    #[default]
    None,
    Named(Vec<IdlField>),
    Tuple(Vec<IdlType>),
}

#[derive(Debug, Deserialize)]
pub struct IdlEnumVariant {
    pub name: String,
    #[serde(default)]
    pub fields: Option<IdlEnumVariantFields>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum IdlEnumVariantFields {
    Named(Vec<IdlField>),
    Tuple(Vec<IdlType>),
}

#[derive(Debug, Deserialize)]
pub struct IdlEvent {
    pub name: String,
    pub discriminator: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct IdlError {
    pub code: u32,
    pub name: String,
    #[serde(default)]
    pub msg: Option<String>,
}
