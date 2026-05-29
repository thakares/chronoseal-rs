use crate::constants::{DEFAULT_GENE_SIZE, MAX_ENV_RECORDS, MAX_GENE_SIZE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentRecord {
    pub symbol: u16,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneState {
    pub gene: Vec<u8>,
    pub environment: Vec<EnvironmentRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneError {
    InvalidGeneSize { size: usize },
    TooManyEnvironmentRecords { len: usize },
    EnvironmentNotSorted,
    DuplicateEnvironmentSymbol(u16),
    ZeroQuantitySymbol(u16),
    EnvironmentFull,
    EnvironmentBlobLengthInvalid { len: usize },
    EnvironmentBlobTooLarge { records: usize },
}

impl std::fmt::Display for GeneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGeneSize { size } => write!(f, "invalid gene size: {size}"),
            Self::TooManyEnvironmentRecords { len } => {
                write!(f, "too many environment records: {len}")
            }
            Self::EnvironmentNotSorted => write!(f, "environment records are not sorted"),
            Self::DuplicateEnvironmentSymbol(symbol) => {
                write!(f, "duplicate environment symbol: {symbol}")
            }
            Self::ZeroQuantitySymbol(symbol) => {
                write!(f, "environment quantity cannot be zero for symbol {symbol}")
            }
            Self::EnvironmentFull => write!(f, "environment is at maximum capacity"),
            Self::EnvironmentBlobLengthInvalid { len } => {
                write!(
                    f,
                    "environment blob length must be a multiple of 6, got {len}"
                )
            }
            Self::EnvironmentBlobTooLarge { records } => {
                write!(f, "environment blob contains too many records: {records}")
            }
        }
    }
}

impl std::error::Error for GeneError {}

pub fn new_state(gene_size: usize) -> Result<GeneState, GeneError> {
    if !(1..=MAX_GENE_SIZE).contains(&gene_size) {
        return Err(GeneError::InvalidGeneSize { size: gene_size });
    }
    Ok(GeneState {
        gene: vec![0; gene_size],
        environment: Vec::new(),
    })
}

pub fn default_state() -> GeneState {
    GeneState {
        gene: vec![0; DEFAULT_GENE_SIZE],
        environment: Vec::new(),
    }
}

pub fn validate_state(state: &GeneState) -> Result<(), GeneError> {
    if !(1..=MAX_GENE_SIZE).contains(&state.gene.len()) {
        return Err(GeneError::InvalidGeneSize {
            size: state.gene.len(),
        });
    }
    validate_environment(&state.environment)
}

pub fn get_env_quantity(state: &GeneState, symbol: u16) -> u32 {
    match state
        .environment
        .binary_search_by_key(&symbol, |record| record.symbol)
    {
        Ok(i) => state.environment[i].quantity,
        Err(_) => 0,
    }
}

pub fn set_env_quantity(
    state: &mut GeneState,
    symbol: u16,
    quantity: u32,
) -> Result<(), GeneError> {
    let idx = state
        .environment
        .binary_search_by_key(&symbol, |record| record.symbol);
    match (idx, quantity) {
        (Ok(i), 0) => {
            state.environment.remove(i);
            Ok(())
        }
        (Ok(i), qty) => {
            state.environment[i].quantity = qty;
            Ok(())
        }
        (Err(_), 0) => Ok(()),
        (Err(i), qty) => {
            if state.environment.len() >= MAX_ENV_RECORDS {
                return Err(GeneError::EnvironmentFull);
            }
            state.environment.insert(
                i,
                EnvironmentRecord {
                    symbol,
                    quantity: qty,
                },
            );
            Ok(())
        }
    }
}

pub fn add_env_quantity(
    state: &mut GeneState,
    symbol: u16,
    quantity: u32,
) -> Result<u32, GeneError> {
    let current = get_env_quantity(state, symbol);
    let next = current.saturating_add(quantity);
    set_env_quantity(state, symbol, next)?;
    Ok(next)
}

pub fn sub_env_quantity(
    state: &mut GeneState,
    symbol: u16,
    quantity: u32,
) -> Result<u32, GeneError> {
    let current = get_env_quantity(state, symbol);
    let next = current.saturating_sub(quantity);
    set_env_quantity(state, symbol, next)?;
    Ok(next)
}

pub fn encode_environment(records: &[EnvironmentRecord]) -> Result<Vec<u8>, GeneError> {
    validate_environment(records)?;
    let mut out = Vec::with_capacity(records.len() * 6);
    for record in records {
        out.extend_from_slice(&record.symbol.to_le_bytes());
        out.extend_from_slice(&record.quantity.to_le_bytes());
    }
    Ok(out)
}

pub fn decode_environment(blob: &[u8]) -> Result<Vec<EnvironmentRecord>, GeneError> {
    if blob.len() % 6 != 0 {
        return Err(GeneError::EnvironmentBlobLengthInvalid { len: blob.len() });
    }
    let records_len = blob.len() / 6;
    if records_len > MAX_ENV_RECORDS {
        return Err(GeneError::EnvironmentBlobTooLarge {
            records: records_len,
        });
    }

    let mut records = Vec::with_capacity(records_len);
    let mut i = 0;
    while i < blob.len() {
        let symbol = u16::from_le_bytes([blob[i], blob[i + 1]]);
        let quantity = u32::from_le_bytes([blob[i + 2], blob[i + 3], blob[i + 4], blob[i + 5]]);
        records.push(EnvironmentRecord { symbol, quantity });
        i += 6;
    }
    validate_environment(&records)?;
    Ok(records)
}

pub fn commitment(state: &GeneState) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"chronoseal/gene/v1");
    h.update(&(state.gene.len() as u32).to_le_bytes());
    h.update(&state.gene);
    h.update(&(state.environment.len() as u16).to_le_bytes());
    for record in &state.environment {
        h.update(&record.symbol.to_le_bytes());
        h.update(&record.quantity.to_le_bytes());
    }
    *h.finalize().as_bytes()
}

pub fn commitment_hex(state: &GeneState) -> String {
    hex::encode(commitment(state))
}

fn validate_environment(records: &[EnvironmentRecord]) -> Result<(), GeneError> {
    if records.len() > MAX_ENV_RECORDS {
        return Err(GeneError::TooManyEnvironmentRecords { len: records.len() });
    }
    let mut prev_symbol: Option<u16> = None;
    for record in records {
        if record.quantity == 0 {
            return Err(GeneError::ZeroQuantitySymbol(record.symbol));
        }
        if let Some(prev) = prev_symbol {
            if record.symbol < prev {
                return Err(GeneError::EnvironmentNotSorted);
            }
            if record.symbol == prev {
                return Err(GeneError::DuplicateEnvironmentSymbol(record.symbol));
            }
        }
        prev_symbol = Some(record.symbol);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    #[test]
    fn test_new_state_with_default_size() {
        let state = new_state(DEFAULT_GENE_SIZE).unwrap();
        assert_eq!(state.gene.len(), DEFAULT_GENE_SIZE);
        assert!(state.environment.is_empty());
    }

    #[test]
    fn test_new_state_rejects_invalid_sizes() {
        assert!(matches!(
            new_state(0).unwrap_err(),
            GeneError::InvalidGeneSize { .. }
        ));
        assert!(matches!(
            new_state(MAX_GENE_SIZE + 1).unwrap_err(),
            GeneError::InvalidGeneSize { .. }
        ));
    }

    #[test]
    fn test_set_and_get_env_quantity() {
        let mut state = new_state(8).unwrap();
        set_env_quantity(&mut state, 42, 7).unwrap();
        assert_eq!(get_env_quantity(&state, 42), 7);
        set_env_quantity(&mut state, 42, 0).unwrap();
        assert_eq!(get_env_quantity(&state, 42), 0);
    }

    #[test]
    fn test_add_env_quantity_saturates() {
        let mut state = new_state(8).unwrap();
        set_env_quantity(&mut state, 1, u32::MAX - 3).unwrap();
        let next = add_env_quantity(&mut state, 1, 99).unwrap();
        assert_eq!(next, u32::MAX);
    }

    #[test]
    fn test_sub_env_quantity_removes_symbol() {
        let mut state = new_state(8).unwrap();
        set_env_quantity(&mut state, 7, 10).unwrap();
        let next = sub_env_quantity(&mut state, 7, 100).unwrap();
        assert_eq!(next, 0);
        assert!(state.environment.is_empty());
    }

    #[test]
    fn test_environment_capacity_limit_is_enforced() {
        let mut state = new_state(8).unwrap();
        for symbol in 0..(MAX_ENV_RECORDS as u16) {
            set_env_quantity(&mut state, symbol, 1).unwrap();
        }
        let err = set_env_quantity(&mut state, 500, 1).unwrap_err();
        assert_eq!(err, GeneError::EnvironmentFull);
    }

    #[test]
    fn test_encode_decode_environment_roundtrip() {
        let records = vec![
            EnvironmentRecord {
                symbol: 3,
                quantity: 9,
            },
            EnvironmentRecord {
                symbol: 11,
                quantity: 999,
            },
        ];
        let blob = encode_environment(&records).unwrap();
        let decoded = decode_environment(&blob).unwrap();
        assert_eq!(decoded, records);
    }

    #[test]
    fn test_decode_environment_rejects_unsorted_records() {
        let mut blob = Vec::new();
        blob.extend_from_slice(&7u16.to_le_bytes());
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.extend_from_slice(&2u16.to_le_bytes());
        blob.extend_from_slice(&1u32.to_le_bytes());
        let err = decode_environment(&blob).unwrap_err();
        assert_eq!(err, GeneError::EnvironmentNotSorted);
    }

    #[test]
    fn test_decode_environment_rejects_zero_quantity() {
        let mut blob = Vec::new();
        blob.extend_from_slice(&9u16.to_le_bytes());
        blob.extend_from_slice(&0u32.to_le_bytes());
        let err = decode_environment(&blob).unwrap_err();
        assert_eq!(err, GeneError::ZeroQuantitySymbol(9));
    }

    #[test]
    fn test_commitment_changes_when_gene_or_environment_changes() {
        let mut state_a = new_state(16).unwrap();
        let mut state_b = state_a.clone();
        assert_eq!(commitment_hex(&state_a), commitment_hex(&state_b));

        state_b.gene[0] = 1;
        assert_ne!(commitment_hex(&state_a), commitment_hex(&state_b));

        set_env_quantity(&mut state_a, 7, 3).unwrap();
        assert_ne!(commitment_hex(&state_a), commitment_hex(&state_b));
    }

    #[test]
    fn test_validate_state_rejects_duplicate_environment_symbols() {
        let state = GeneState {
            gene: vec![0; 10],
            environment: vec![
                EnvironmentRecord {
                    symbol: 1,
                    quantity: 1,
                },
                EnvironmentRecord {
                    symbol: 1,
                    quantity: 2,
                },
            ],
        };
        assert_eq!(
            validate_state(&state).unwrap_err(),
            GeneError::DuplicateEnvironmentSymbol(1)
        );
    }

    #[test]
    fn test_table_driven_randomized_environment_roundtrip() {
        for seed in 0..32u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let mut state = new_state(32).unwrap();

            for _ in 0..128 {
                let symbol = rng.gen_range(0u16..200u16);
                let qty = if rng.gen_bool(0.15) {
                    0
                } else {
                    rng.gen_range(1u32..100_000u32)
                };
                if let Err(err) = set_env_quantity(&mut state, symbol, qty) {
                    assert_eq!(err, GeneError::EnvironmentFull);
                }
                validate_state(&state).unwrap();
            }

            let blob = encode_environment(&state.environment).unwrap();
            let decoded = decode_environment(&blob).unwrap();
            assert_eq!(decoded, state.environment);

            let commitment_a = commitment(&state);
            let commitment_b = commitment(&state.clone());
            assert_eq!(commitment_a, commitment_b);
        }
    }
}
