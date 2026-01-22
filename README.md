# Solana IDL Parser

A Rust procedural macro that generates type-safe code from Solana Anchor IDL (Interface Definition Language) files at compile time.

## Overview

This library eliminates boilerplate when working with Solana programs by automatically generating Rust types from Anchor IDL JSON files. Instead of manually writing deserialization code and type definitions, simply point the macro at your IDL file and get fully-typed, ready-to-use structs and enums.

## Usage

```rust
use solana_idl_parser::parse_idl;

// Parse IDL and generate all types
parse_idl!("idl/program.json");

// Use generated types to deserialize instruction data
let instruction = ProgramInstructions::deserialize(accounts, data)?;

match instruction {
    ProgramInstructions::Initialize(accounts, args) => {
        println!("Authority: {}", accounts.authority);
        println!("Initial amount: {}", args.amount);
    }
    ProgramInstructions::Update(accounts, args) => {
        println!("Updating with new value: {}", args.new_value);
    }
    // ...
}
```

## What Gets Generated

For each instruction in your IDL, the macro generates:

### 1. Discriminator Constants
```rust
pub const INITIALIZE_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
```

### 2. Accounts Structs
```rust
#[derive(Copy, Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct InitializeAccounts {
    pub authority: Pubkey,
    pub system_program: Pubkey,
}

impl InitializeAccounts {
    pub fn from_account_metas(metas: &[AccountMeta]) -> anyhow::Result<Self> {
        // Validation and conversion logic
    }
}
```

### 3. Args Structs
```rust
#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct InitializeArgs {
    pub amount: u64,
    pub config: ConfigParams,
}
```

### 4. Instructions Enum
```rust
#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub enum ProgramInstructions {
    Initialize(InitializeAccounts, InitializeArgs),
    Update(UpdateAccounts, UpdateArgs),
    Close(CloseAccounts),
}
```

### 5. Custom Types
All type definitions from the IDL are generated with proper Borsh derives:
```rust
#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub struct ConfigParams {
    pub fee_rate: u16,
    pub max_supply: u64,
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub enum Status {
    Active,
    Paused,
    Closed,
}
```

## Type Mapping

The parser intelligently maps IDL types to their Rust equivalents:

| IDL Type | Rust Type | Notes |
|----------|-----------|-------|
| `bool`, `u8`, `u16`, `u32`, `u64`, `u128` | Native types | Direct mapping |
| `i8`, `i16`, `i32`, `i64`, `i128` | Native types | Signed integers |
| `f32`, `f64` | Native types | Floating point |
| `string` | `String` | Heap-allocated string |
| `pubkey` | `solana_sdk::pubkey::Pubkey` | Solana public key |
| `bytes` | `Vec<u8>` | Dynamic byte array |
| `{ "option": T }` | `Option<T>` | Optional values |
| `{ "vec": T }` | `Vec<T>` | Dynamic arrays |
| `{ "array": [T, N] }` | `[T; N]` | Fixed-size arrays |
| `{ "defined": "CustomType" }` | `CustomType` | User-defined types |


## License

MIT