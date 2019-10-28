use std::str::FromStr;
use std::collections::BTreeMap;
use primitive_types::{U256, H256, H160};
use evm::backend::MemoryAccount;

pub fn u256_to_h256(u: U256) -> H256 {
	let mut h = H256::default();
	u.to_big_endian(&mut h[..]);
	h
}

pub fn unwrap_to_u256(s: &str) -> U256 {
	if s.starts_with("0x") {
		U256::from_str(&s[2..]).unwrap()
	} else {
		U256::from_dec_str(s).unwrap()
	}
}

pub fn unwrap_to_h256(s: &str) -> H256 {
	assert!(s.starts_with("0x"));
	H256::from_str(&s[2..]).unwrap()
}

pub fn unwrap_to_h160(s: &str) -> H160 {
	assert!(s.starts_with("0x"));
	H160::from_str(&s[2..]).unwrap()
}

pub fn unwrap_to_vec(s: &str) -> Vec<u8> {
	assert!(s.starts_with("0x"));
	hex::decode(&s[2..]).unwrap()
}

pub fn unwrap_to_account(s: &ethjson::spec::Account) -> MemoryAccount {
	MemoryAccount {
		balance: s.balance.clone().unwrap().into(),
		nonce: s.nonce.clone().unwrap().into(),
		code: s.code.clone().unwrap().into(),
		storage: s.storage.as_ref().unwrap().iter().map(|(k, v)| {
			(u256_to_h256(k.clone().into()), u256_to_h256(v.clone().into()))
		}).collect(),
	}
}

pub fn unwrap_to_state(a: &ethjson::spec::State) -> BTreeMap<H160, MemoryAccount> {
	match &a.0 {
		ethjson::spec::HashOrMap::Map(m) => {
			m.iter().map(|(k, v)| {
				(k.clone().into(), unwrap_to_account(v))
			}).collect()
		},
		ethjson::spec::HashOrMap::Hash(_) => panic!("Hash can not be converted."),
	}
}

pub fn assert_valid_state(a: &ethjson::spec::State, b: &BTreeMap<H160, MemoryAccount>) {
	match &a.0 {
		ethjson::spec::HashOrMap::Map(m) => {
			assert_eq!(
				&m.iter().map(|(k, v)| {
					(k.clone().into(), unwrap_to_account(v))
				}).collect::<BTreeMap<_, _>>(),
				b
			);
		},
		ethjson::spec::HashOrMap::Hash(h) => unimplemented!(),
	}
}
