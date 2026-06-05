use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use isotropy_protocol::msg::{
    BurnQuoteResponse, ConfigResponse, CurrentCycleResponse, CycleResponse, EmissionPointResponse,
    ExecuteMsg, GlobalStateResponse, InstantiateMsg, MigrateMsg, PositionResponse, QueryMsg,
};

fn main() {
    let mut out_dir = current_dir().expect("current dir");
    out_dir.push("schema");

    create_dir_all(&out_dir).expect("create schema dir");
    remove_schemas(&out_dir).expect("clean schema dir");

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(MigrateMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(GlobalStateResponse), &out_dir);
    export_schema(&schema_for!(BurnQuoteResponse), &out_dir);
    export_schema(&schema_for!(PositionResponse), &out_dir);
    export_schema(&schema_for!(CycleResponse), &out_dir);
    export_schema(&schema_for!(CurrentCycleResponse), &out_dir);
    export_schema(&schema_for!(EmissionPointResponse), &out_dir);
}
