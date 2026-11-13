use canteiro_calculation_verification::{run_case, verify_calculation_vector, verify_document};
use serde_json::{Value, json};

const REFERENCE: &str = include_str!("../../../docs/validacao/issue-005/reference-vectors.json");
const SPECIFICATION: &str = include_str!("../../../docs/validacao/vetores-calculo.json");

#[test]
fn independent_determinant_and_property_samples_match_the_domain() {
    let document: Value = serde_json::from_str(REFERENCE).unwrap();
    let actual: Value = serde_json::from_str(&verify_document(REFERENCE)).unwrap();
    let cases = document["vectors"].as_array().unwrap().iter().chain(
        document["property_samples"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|group| group["samples"].as_array().unwrap()),
    );
    let actual = actual.as_array().unwrap();
    let mut count = 0;
    for (case, actual) in cases.zip(actual) {
        assert_eq!(actual["id"], case["id"]);
        assert_eq!(actual["result"], case["expected"], "{}", case["id"]);
        let single = verify_calculation_vector(&case.to_string());
        assert_eq!(single, actual["result"].to_string(), "{}", case["id"]);
        count += 1;
    }
    assert_eq!(count, 74);
    assert_eq!(actual.len(), count);
}

#[test]
fn expected_fields_cannot_supply_or_change_the_calculated_answer() {
    let mut request = json!({
        "id": "transport-independence",
        "operation": "unit_measurement",
        "inputs": {"contracted":"100","previous":"20","current":"35","unit_price":"10"},
        "expected": {"amount":"999999999.99"},
    });
    let output = run_case(&request);
    assert_eq!(output, json!({"quantity":"15.000000","amount":"150.00"}));
    request["expected"] = Value::Null;
    assert_eq!(run_case(&request), output);
    request.as_object_mut().unwrap().remove("expected");
    assert_eq!(run_case(&request), output);
    request["inputs"]["current"] = json!("36");
    assert_eq!(run_case(&request)["amount"], "160.00");
    assert_eq!(
        verify_calculation_vector("invalid json"),
        "{\"error\":\"Format\"}"
    );
}

#[test]
fn original_specification_numeric_and_calendar_examples_still_hold() {
    let document: Value = serde_json::from_str(SPECIFICATION).unwrap();
    let vectors = document["vectors"].as_array().unwrap();
    let by_id = |id: &str| vectors.iter().find(|vector| vector["id"] == id).unwrap();

    let global = by_id("V03");
    assert_eq!(
        run_case(&json!({"operation":"global_measurement","inputs":global["inputs"]})),
        global["expected"],
    );
    let adjustment = by_id("V05");
    assert_eq!(
        run_case(&json!({"operation":"adjustment","inputs":adjustment["inputs"]})),
        adjustment["expected"],
    );
    let complement = by_id("V06");
    assert_eq!(
        run_case(&json!({"operation":"arithmetic","inputs":{
            "method":"sub","left":complement["inputs"]["definitive"],
            "right":complement["inputs"]["recognized"],"output_scale":2,
        }}))["value"],
        complement["expected"]["delta"],
    );
    for (id, anchor, policy, cadence, periods) in [
        ("V13", "2027-01-31", "civil", "monthly", json!([1, 2])),
        ("V14", "2028-01-31", "civil", "monthly", json!([1, 2])),
    ] {
        assert_eq!(
            run_case(&json!({"operation":"calendar_sequence","inputs":{
                "anchor":anchor,"policy":policy,"cadence":cadence,"periods":periods,
            }})),
            by_id(id)["expected"],
        );
    }
}
