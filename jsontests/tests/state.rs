use std::path::PathBuf;
use std::fs::{self, File};
use std::io::BufReader;
use std::collections::HashMap;
use evm_jsontests::state as statetests;

pub fn run(dir: &str) {
	let mut dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	dest.push(dir);

	for entry in fs::read_dir(dest).unwrap() {
		let entry = entry.unwrap();
        let path = entry.path();

		let file = File::open(path).expect("Open file failed");

		let reader = BufReader::new(file);
		let coll = serde_json::from_reader::<_, HashMap<String, statetests::Test>>(reader)
			.expect("Parse test cases failed");

		for (name, test) in coll {
			statetests::test(&name, test);
		}
	}
}

#[test] fn st_bad_opcode() { run("res/ethtests/GeneralStateTests/stBadOpcode"); }
