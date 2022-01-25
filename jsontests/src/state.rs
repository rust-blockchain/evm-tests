use crate::mock::{deposit, get_state, new_test_ext, setup_state, withdraw, Runtime};
use crate::utils::*;
use ethjson::spec::ForkSpec;
use evm_utility::evm::{backend::MemoryAccount, Config, ExitError, ExitSucceed};
use lazy_static::lazy_static;
use module_evm::{
	runner::state::{PrecompileFn, PrecompileOutput},
	Context, StackExecutor, StackSubstateMetadata, SubstrateStackState, Vicinity,
};
use parity_crypto::publickey;
use primitive_types::{H160, H256, U256};
use primitives::convert_decimals_to_evm;
use serde::Deserialize;
use sp_runtime::SaturatedConversion;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::sync::Mutex;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::test_helpers::state::State);

impl Test {
	pub fn unwrap_to_gas_limit(&self) -> u64 {
		self.0.env.gas_limit.into()
	}
	pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
		unwrap_to_state(&self.0.pre_state)
	}

	pub fn unwrap_caller(&self) -> H160 {
		let secret_key: H256 = self.0.transaction.secret.clone().unwrap().into();
		let secret = publickey::Secret::import_key(&secret_key[..]).unwrap();
		let public = publickey::KeyPair::from_secret(secret)
			.unwrap()
			.public()
			.clone();
		let sender = publickey::public_to_address(&public);

		sender
	}

	pub fn unwrap_block_base_fee_per_gas(&self) -> U256 {
		self.0.env.block_base_fee_per_gas.0
	}

	pub fn unwrap_to_vicinity(&self, _spec: &ForkSpec) -> Option<Vicinity> {
		let block_base_fee_per_gas = self.0.env.block_base_fee_per_gas.0;
		let gas_price = self.0.transaction.gas_price.0;

		// gas price cannot be lower than base fee
		if gas_price < block_base_fee_per_gas {
			return None;
		}

		Some(Vicinity {
			gas_price,
			origin: self.unwrap_caller(),
			// block_hashes: Vec::new(),
			// block_number: self.0.env.number.clone().into(),
			block_coinbase: Some(self.0.env.author.clone().into()),
			// block_timestamp: self.0.env.timestamp.clone().into(),
			block_difficulty: Some(self.0.env.difficulty.clone().into()),
			block_gas_limit: Some(self.0.env.gas_limit.clone().into()),
			// chain_id: U256::one(),
			// block_base_fee_per_gas,
		})
	}
}

lazy_static! {
	static ref ISTANBUL_BUILTINS: BTreeMap<H160, ethcore_builtin::Builtin> =
		JsonPrecompile::builtins("./res/istanbul_builtins.json");
}

lazy_static! {
	static ref BERLIN_BUILTINS: BTreeMap<H160, ethcore_builtin::Builtin> =
		JsonPrecompile::builtins("./res/berlin_builtins.json");
}

lazy_static! {
	static ref PRECOMPILE_LIST: Mutex<BTreeMap<H160, PrecompileFn>> = Mutex::new(BTreeMap::new());
}

macro_rules! precompile_entry {
	($map:expr, $builtins:expr, $index:expr) => {
		let x: fn(
			H160,
			&[u8],
			Option<u64>,
			&Context,
		) -> Option<Result<PrecompileOutput, ExitError>> =
			|_addr: H160, input: &[u8], gas_limit: Option<u64>, _context: &Context| {
				let builtin = $builtins.get(&H160::from_low_u64_be($index)).unwrap();
				Some(Self::exec_as_precompile(builtin, input, gas_limit))
			};
		$map.insert(H160::from_low_u64_be($index), x);
	};
}

pub struct JsonPrecompile(BTreeMap<H160, PrecompileFn>);

impl JsonPrecompile {
	pub fn precompile(spec: &ForkSpec) {
		let mut map = PRECOMPILE_LIST.lock().unwrap();
		match spec {
			ForkSpec::Istanbul => {
				precompile_entry!(map, ISTANBUL_BUILTINS, 1);
				precompile_entry!(map, ISTANBUL_BUILTINS, 2);
				precompile_entry!(map, ISTANBUL_BUILTINS, 3);
				precompile_entry!(map, ISTANBUL_BUILTINS, 4);
				precompile_entry!(map, ISTANBUL_BUILTINS, 5);
				precompile_entry!(map, ISTANBUL_BUILTINS, 6);
				precompile_entry!(map, ISTANBUL_BUILTINS, 7);
				precompile_entry!(map, ISTANBUL_BUILTINS, 8);
				precompile_entry!(map, ISTANBUL_BUILTINS, 9);
			}
			ForkSpec::Berlin => {
				precompile_entry!(map, BERLIN_BUILTINS, 1);
				precompile_entry!(map, BERLIN_BUILTINS, 2);
				precompile_entry!(map, BERLIN_BUILTINS, 3);
				precompile_entry!(map, BERLIN_BUILTINS, 4);
				precompile_entry!(map, BERLIN_BUILTINS, 5);
				precompile_entry!(map, BERLIN_BUILTINS, 6);
				precompile_entry!(map, BERLIN_BUILTINS, 7);
				precompile_entry!(map, BERLIN_BUILTINS, 8);
				precompile_entry!(map, BERLIN_BUILTINS, 9);
			}
			// precompiles for London and Berlin are the same
			ForkSpec::London => Self::precompile(&ForkSpec::Berlin),
			_ => {}
		}
	}

	fn builtins(spec_path: &str) -> BTreeMap<H160, ethcore_builtin::Builtin> {
		let reader = std::fs::File::open(spec_path).unwrap();
		let builtins: BTreeMap<ethjson::hash::Address, ethjson::spec::builtin::BuiltinCompat> =
			serde_json::from_reader(reader).unwrap();
		builtins
			.into_iter()
			.map(|(address, builtin)| {
				(
					address.into(),
					ethjson::spec::Builtin::from(builtin).try_into().unwrap(),
				)
			})
			.collect()
	}

	pub fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
	) -> Option<core::result::Result<PrecompileOutput, ExitError>> {
		let map = PRECOMPILE_LIST.lock().unwrap();
		if let Some(x) = map.get(&address) {
			return x(address, input, target_gas, context);
		}
		None
	}

	fn exec_as_precompile(
		builtin: &ethcore_builtin::Builtin,
		input: &[u8],
		gas_limit: Option<u64>,
	) -> Result<PrecompileOutput, ExitError> {
		let cost = builtin.cost(input, 0);

		if let Some(target_gas) = gas_limit {
			if cost > U256::from(u64::MAX) || target_gas < cost.as_u64() {
				return Err(ExitError::OutOfGas);
			}
		}

		let mut output = Vec::new();
		match builtin.execute(input, &mut parity_bytes::BytesRef::Flexible(&mut output)) {
			Ok(()) => Ok(PrecompileOutput {
				exit_status: ExitSucceed::Stopped,
				output,
				cost: cost.as_u64(),
				logs: Vec::new(),
			}),
			Err(e) => Err(ExitError::Other(e.into())),
		}
	}
}

pub fn test(name: &str, test: Test) {
	use std::thread;

	const STACK_SIZE: usize = 16 * 1024 * 1024;

	let name = name.to_string();
	// Spawn thread with explicit stack size
	let child = thread::Builder::new()
		.stack_size(STACK_SIZE)
		.spawn(move || test_run(&name, test))
		.unwrap();

	// Wait for thread to join
	child.join().unwrap();
}

lazy_static! {
	static ref SKIP_NAMES: Vec<&'static str> = vec![
		"HighGasPrice",
		"TestStoreGasPrices",
		"modexp_modsize0_returndatasize",
		"ABAcalls2",
		"CallRecursiveBomb",
		"Call1024",
		"Delegatecall1024",
		"Callcode1024",
		// more state than expected
		"ZeroValue_TransactionCALL_ToOneStorageKey",
		"ZeroValue_TransactionCALLwithData_ToEmpty",
		"ZeroValue_TransactionCALL_ToEmpty",
		"ZeroValue_CALL_ToOneStorageKey",
		"ZeroValue_CALL_ToEmpty",
		"ZeroValue_SUICIDE_ToEmpty",
		"ZeroValue_SUICIDE_ToOneStorageKey",
		"ZeroValue_TransactionCALLwithData_ToOneStorageKey",
		"callToEmptyThenCallError",
		"failed_tx_xcf416c53"
	];
}

fn test_run(name: &str, test: Test) {
	// skip those tests until fixed
	for x in SKIP_NAMES.iter() {
		if name.starts_with(x) {
			return;
		}
	}

	// if name != "SelfDestruct" { return; }
	for (spec, states) in &test.0.post_states {
		new_test_ext().execute_with(|| {
			let (gasometer_config, _delete_empty) = match spec {
				ethjson::spec::ForkSpec::Istanbul => (Config::istanbul(), true),
				// ethjson::spec::ForkSpec::Berlin => (Config::berlin(), true),
				// ethjson::spec::ForkSpec::London => (Config::london(), true),
				_spec => {
					// println!("Skip spec {:?}", spec);
					return;
				}
			};

			let original_state = test.unwrap_to_pre_state();

			let vicinity = test.unwrap_to_vicinity(spec);
			if vicinity.is_none() {
				// if vicinity could not be computed then the transaction was invalid so we simply
				// check the original state and move on
				assert_valid_hash(&states.first().unwrap().hash.0, &original_state);
				return;
			}

			let vicinity = vicinity.unwrap();
			let caller = test.unwrap_caller();
			let caller_balance = original_state.get(&caller).unwrap().balance;

			for (i, state) in states.iter().enumerate() {
				println!("Running {}:{:?}:{} ... ", name, spec, i);
				flush();

				let transaction = test.0.transaction.select(&state.indexes);

				// Only execute valid transactions
				if let Ok(transaction) = crate::utils::transaction::validate(
					transaction,
					test.0.env.gas_limit.0,
					caller_balance,
					&gasometer_config,
				) {
					setup_state(
						original_state.clone(),
						test.0.env.number.0.as_u64(),
						test.0.env.timestamp.0.as_u64(),
					);

					let gas_limit: u64 = transaction.gas_limit.into();
					let data: Vec<u8> = transaction.data.into();

					let metadata =
						StackSubstateMetadata::new(gas_limit, 1_000_000, &gasometer_config);

					let stack_state = SubstrateStackState::<Runtime>::new(&vicinity, metadata);

					JsonPrecompile::precompile(spec);

					let mut executor = StackExecutor::new_with_precompile(
						stack_state,
						&gasometer_config,
						JsonPrecompile::execute,
					);

					let total_fee = (vicinity.gas_price * gas_limit).saturated_into::<i128>();
					withdraw(caller, total_fee);

					let access_list = transaction
						.access_list
						.into_iter()
						.map(|(address, keys)| (address.0, keys.into_iter().map(|k| k.0).collect()))
						.collect();

					match transaction.to {
						ethjson::maybe::MaybeEmpty::Some(to) => {
							let data = data;
							let value: U256 = transaction.value.into();

							let _reason = executor.transact_call(
								caller,
								to.into(),
								convert_decimals_to_evm(value.saturated_into::<u128>()).into(),
								data,
								gas_limit,
								access_list,
							);
						}
						ethjson::maybe::MaybeEmpty::None => {
							let code = data;
							let value: U256 = transaction.value.into();

							let _reason = executor.transact_create(
								caller,
								convert_decimals_to_evm(value.saturated_into::<u128>()).into(),
								code,
								gas_limit,
								access_list,
							);
						}
					}

					let actual_fee = executor.fee(vicinity.gas_price).saturated_into::<i128>();
					deposit(vicinity.block_coinbase.unwrap(), actual_fee);
					println!("Gas used: {}", executor.used_gas());

					let refund_fee = total_fee - actual_fee;
					deposit(caller, refund_fee);

					if let Some(post_state) = state.post_state.clone() {
						let expected_state = post_state
							.into_iter()
							.map(|(acc, data)| (acc.into(), unwrap_to_account(&data)))
							.collect::<BTreeMap<H160, MemoryAccount>>();
						let actual_state = get_state(&executor.into_state());
						assert_states(expected_state, actual_state);
					} else {
						eprintln!("No post state found!");
						// assert_valid_hash(&state.hash.0, &get_state(&executor.into_state()));
					}

					// clear
					module_evm::Accounts::<Runtime>::remove_all(None);
					module_evm::AccountStorages::<Runtime>::remove_all(None);
					module_evm::Codes::<Runtime>::remove_all(None);
					module_evm::CodeInfos::<Runtime>::remove_all(None);
					module_evm::ContractStorageSizes::<Runtime>::remove_all(None);
					frame_system::Account::<Runtime>::remove_all(None);
				}

				println!("passed");
			}
		});
	}
}

fn assert_states(a: BTreeMap<H160, MemoryAccount>, b: BTreeMap<H160, MemoryAccount>) {
	let mut b = b;
	a.into_iter().for_each(|(address, a_account)| {
		let maybe_b_account = b.get(&address);
		assert!(
			maybe_b_account.is_some(),
			"address {:?} not found in b states",
			address
		);
		let b_account = maybe_b_account.unwrap();
		// assert_eq!(a_account.balance, b_account.balance, "balance not eq for address {:?}", address);
		assert_eq!(
			a_account.nonce, b_account.nonce,
			"nonce not eq for address {:?}",
			address
		);
		assert_eq!(
			a_account.storage, b_account.storage,
			"storage not eq for address {:?}",
			address
		);
		assert_eq!(
			a_account.code, b_account.code,
			"code not eq for address {:?}",
			address
		);
		b.remove(&address);
	});
	assert!(b.is_empty(), "unexpected state {:?}", b);
}
