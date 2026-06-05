use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use cw20::BalanceResponse;
use cw20_base::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

fn main() {
    let mut out_dir = current_dir().expect("current dir");
    out_dir.push("schema");

    create_dir_all(&out_dir).expect("create schema dir");
    remove_schemas(&out_dir).expect("clean schema dir");

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(BalanceResponse), &out_dir);
}
