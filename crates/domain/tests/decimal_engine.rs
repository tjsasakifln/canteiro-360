use canteiro_domain::decimal::{
    AllocationWeight, CalculationPolicy, DecimalError, ExactDecimal, Factor, IndexValue, Money,
    Percent, Quantity, UnitPrice, adjustment, allocate, allocate_money, allocate_percentages,
    global_measurement, negotiated_unit_price, unit_measurement, unit_measurement_with_previous,
};

fn decimal(value: &str) -> ExactDecimal {
    value.parse().unwrap()
}

fn weights(rows: &[(&str, u32, &str)]) -> Vec<AllocationWeight> {
    rows.iter()
        .map(|&(id, position, value)| AllocationWeight {
            id: id.into(),
            position,
            weight: decimal(value),
        })
        .collect()
}

#[test]
fn canonical_text_and_deserialization_never_guess_a_convention() {
    for value in [
        "", " 1", "1 ", "+1", "01", "-00.1", ".1", "1.", "1,2", "1.000,00", "1e2", "NaN", "-0",
        "-0.000",
    ] {
        assert_eq!(
            value.parse::<ExactDecimal>(),
            Err(DecimalError::Format),
            "{value}"
        );
    }
    assert_eq!(decimal("1234.560000").to_string(), "1234.56");
    assert_eq!(decimal("-0.335").to_string(), "-0.335");
    assert_eq!(
        serde_json::to_string(&decimal("1234.56")).unwrap(),
        "\"1234.56\""
    );
    assert!(serde_json::from_str::<ExactDecimal>("1234.56").is_err());
    assert!(serde_json::from_str::<Money>("0.33").is_err());
    assert!(serde_json::from_str::<Quantity>("\"-1\"").is_err());
    assert!(serde_json::from_str::<Money>("\"0.001\"").is_err());
    assert!(serde_json::from_str::<CalculationPolicy>("\"C360-DECIMAL-2\"").is_err());
    assert_eq!(
        CalculationPolicy::require("C360-DECIMAL-0"),
        Err(DecimalError::UnknownPolicy)
    );
}

#[test]
fn field_scales_and_numeric_column_bounds_are_enforced_before_rounding() {
    for text in ["0.0000000", "1.1234567"] {
        assert_eq!(text.parse::<Quantity>(), Err(DecimalError::Scale));
        assert_eq!(text.parse::<UnitPrice>(), Err(DecimalError::Scale));
        assert_eq!(text.parse::<Percent>(), Err(DecimalError::Scale));
    }
    assert_eq!("0.000".parse::<Money>(), Err(DecimalError::Scale));
    assert_eq!(
        "0.00000000000".parse::<IndexValue>(),
        Err(DecimalError::Scale)
    );
    assert!("99999999999999.999999".parse::<Quantity>().is_ok());
    assert!("99999999999999.999999".parse::<UnitPrice>().is_ok());
    assert!("999999999999999999.99".parse::<Money>().is_ok());
    assert!("-999999999999999999.99".parse::<Money>().is_ok());
    assert!("999999.999999".parse::<Percent>().is_ok());
    assert!("9999999999.9999999999".parse::<IndexValue>().is_ok());
    assert_eq!(
        "100000000000000".parse::<Quantity>(),
        Err(DecimalError::Overflow)
    );
    assert_eq!(
        "100000000000000".parse::<UnitPrice>(),
        Err(DecimalError::Overflow)
    );
    assert_eq!(
        "1000000000000000000".parse::<Money>(),
        Err(DecimalError::Overflow)
    );
    assert_eq!("1000000".parse::<Percent>(), Err(DecimalError::Overflow));
    assert_eq!(
        "10000000000".parse::<IndexValue>(),
        Err(DecimalError::Overflow)
    );
    assert_eq!("0".parse::<IndexValue>(), Err(DecimalError::Range));
    assert!("0".parse::<Factor>().is_ok());
    assert_eq!("-1".parse::<Quantity>(), Err(DecimalError::Negative));
    assert_eq!("-1".parse::<UnitPrice>(), Err(DecimalError::Negative));
    assert_eq!("-1".parse::<Percent>(), Err(DecimalError::Negative));
    assert_eq!("-1".parse::<IndexValue>(), Err(DecimalError::Negative));
}

#[test]
fn exact_operations_refuse_hidden_rounding_and_unsupported_precision() {
    assert_eq!(
        decimal("1").checked_div_exact(decimal("8")).unwrap(),
        decimal("0.125")
    );
    assert_eq!(
        decimal("1").checked_div_exact(decimal("3")),
        Err(DecimalError::Precision)
    );
    assert_eq!(
        decimal("0").checked_div_exact(decimal("0")),
        Err(DecimalError::DivisionByZero)
    );
    assert_eq!(
        decimal("1").divide_rounded(decimal("0"), 10),
        Err(DecimalError::DivisionByZero)
    );
    assert_eq!(
        decimal("9999999999999999999999999999").checked_add(decimal("1")),
        Err(DecimalError::Overflow)
    );
    assert_eq!(
        decimal("0.0000000000000000000000000001").checked_mul(decimal("0.1")),
        Err(DecimalError::Precision)
    );
    let high_precision = decimal("1.234567891234567891234567891");
    assert_eq!(
        high_precision.checked_mul(high_precision),
        Err(DecimalError::Precision)
    );
    assert_eq!(decimal("0.33").fixed_text(1), Err(DecimalError::Precision));
    assert_eq!(decimal("0.33").fixed_text(29), Err(DecimalError::Scale));
    assert_eq!(decimal("0.33").fixed_text(2).unwrap(), "0.33");
    assert_eq!(decimal("0.33").fixed_text(6).unwrap(), "0.330000");
}

#[test]
fn scratch_integer_capacity_does_not_reject_an_exact_representable_product() {
    // 2^90 * 5^39 = 2^51 * 10^39. The unnormalized coefficient needs
    // 55 digits, but its exact decimal result has only 16 significant digits.
    let a = decimal("0.1237940039285380274899124224");
    let b = decimal("0.1818989403545856475830078125");
    assert_eq!(a.checked_mul(b).unwrap(), decimal("0.02251799813685248"));
    assert_eq!(
        decimal("1.00")
            .checked_div_exact(decimal("0.0001"))
            .unwrap(),
        decimal("10000")
    );
}

#[test]
fn truncation_and_nearest_rounding_use_the_documented_signed_rules() {
    for (value, truncated, rounded) in [
        ("0.335", "0.33", "0.34"),
        ("-0.335", "-0.33", "-0.34"),
        ("9.995", "9.99", "10.00"),
        ("-9.995", "-9.99", "-10.00"),
        ("0.0049", "0.00", "0.00"),
        ("0.005", "0.00", "0.01"),
    ] {
        assert_eq!(
            decimal(value).truncate(2).unwrap().fixed_text(2).unwrap(),
            truncated
        );
        assert_eq!(
            decimal(value).round_away(2).unwrap().fixed_text(2).unwrap(),
            rounded
        );
    }
    assert_eq!(
        decimal("1")
            .divide_rounded(decimal("6"), 10)
            .unwrap()
            .fixed_text(10)
            .unwrap(),
        "0.1666666667"
    );
    assert_eq!(
        decimal("-1")
            .divide_rounded(decimal("8"), 2)
            .unwrap()
            .fixed_text(2)
            .unwrap(),
        "-0.13"
    );
    assert_eq!(
        decimal("1")
            .divide_rounded(decimal("-8"), 2)
            .unwrap()
            .fixed_text(2)
            .unwrap(),
        "-0.13"
    );
    assert_eq!(
        decimal("-1")
            .divide_rounded(decimal("-8"), 2)
            .unwrap()
            .fixed_text(2)
            .unwrap(),
        "0.13"
    );
}

#[test]
fn centavos_are_preserved_across_cumulative_measurements() {
    let mut previous = "0".parse().unwrap();
    let mut recognized = "0".parse().unwrap();
    for (current, amount) in [("1", "0.33"), ("2", "0.34"), ("3", "0.33")] {
        let result = unit_measurement_with_previous(
            "3".parse().unwrap(),
            previous,
            current.parse().unwrap(),
            "0.335".parse().unwrap(),
            recognized,
        )
        .unwrap();
        assert_eq!(result.period_value.to_string(), amount);
        previous = current.parse().unwrap();
        recognized = result.current_accumulated;
    }
    assert_eq!(recognized.to_string(), "1.00");
    let result = unit_measurement(
        "100".parse().unwrap(),
        "20".parse().unwrap(),
        "35".parse().unwrap(),
        "10".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(result.quantity.to_string(), "15");
    assert_eq!(result.period_value.to_string(), "150.00");
    assert_eq!(result.remaining_quantity.to_string(), "65");
    assert_eq!(result.remaining_value.to_string(), "650.00");
}

#[test]
fn measurement_refuses_overrun_unapproved_reduction_and_inconsistent_history() {
    let result = |previous: &str, current: &str| {
        unit_measurement(
            "100".parse().unwrap(),
            previous.parse().unwrap(),
            current.parse().unwrap(),
            "10".parse().unwrap(),
        )
    };
    assert_eq!(result("20", "101"), Err(DecimalError::InsufficientBalance));
    assert_eq!(result("101", "101"), Err(DecimalError::InsufficientBalance));
    assert_eq!(result("20", "15"), Err(DecimalError::Decreasing));
    assert_eq!(
        unit_measurement_with_previous(
            "100".parse().unwrap(),
            "20".parse().unwrap(),
            "35".parse().unwrap(),
            "10".parse().unwrap(),
            "199.99".parse().unwrap()
        ),
        Err(DecimalError::InconsistentPrevious)
    );
    assert_eq!(result("20", "20").unwrap().period_value.to_string(), "0.00");
}

#[test]
fn negotiated_price_uses_one_bdi_one_discount_and_explicit_rounding() {
    let result = |cost: &str, bdi: &str, discount: &str| {
        negotiated_unit_price(
            cost.parse().unwrap(),
            bdi.parse().unwrap(),
            discount.parse().unwrap(),
        )
    };
    assert_eq!(result("100", "20", "10").unwrap().to_string(), "108.000000");
    assert_eq!(
        result("0.000005", "10", "0").unwrap().to_string(),
        "0.000006"
    );
    assert_eq!(
        result("100", "1000", "0").unwrap().to_string(),
        "1100.000000"
    );
    assert_eq!(result("100", "20", "100").unwrap().to_string(), "0.000000");
    assert_eq!(result("100", "1000.000001", "0"), Err(DecimalError::Range));
    assert_eq!(result("100", "20", "100.000001"), Err(DecimalError::Range));
}

#[test]
fn global_stage_closes_exactly_and_rejects_invalid_progress() {
    let result = |stage: &str, previous: &str, current: &str| {
        global_measurement(
            stage.parse().unwrap(),
            previous.parse().unwrap(),
            current.parse().unwrap(),
        )
    };
    assert_eq!(
        result("100000", "20", "45")
            .unwrap()
            .period_value
            .to_string(),
        "25000.00"
    );
    assert_eq!(
        result("0.01", "50", "100")
            .unwrap()
            .period_value
            .to_string(),
        "0.01"
    );
    assert_eq!(result("100", "20", "101"), Err(DecimalError::Range));
    assert_eq!(result("100", "20", "19"), Err(DecimalError::Decreasing));
    assert_eq!(result("-100", "0", "10"), Err(DecimalError::Negative));
}

#[test]
fn adjustment_uses_a_ten_place_factor_and_signed_truncation() {
    let result = |base: &str, index_base: &str, index_current: &str| {
        adjustment(
            base.parse().unwrap(),
            index_base.parse().unwrap(),
            index_current.parse().unwrap(),
        )
    };
    let positive = result("10000", "100", "105").unwrap();
    assert_eq!(positive.factor.to_string(), "1.0500000000");
    assert_eq!(positive.amount.to_string(), "500.00");
    assert_eq!(
        result("10000", "100", "95").unwrap().amount.to_string(),
        "-500.00"
    );
    let tiny = result("100", "9999999999.9999999999", "0.0000000001").unwrap();
    assert_eq!(tiny.factor.to_string(), "0.0000000000");
    assert_eq!(tiny.amount.to_string(), "-100.00");
    assert_eq!(result("-1", "100", "105"), Err(DecimalError::Negative));
}

#[test]
fn largest_remainders_conserve_money_and_percentages_with_stable_ties() {
    let rows = weights(&[("c", 3, "1"), ("b", 2, "1"), ("a", 1, "1")]);
    let allocation = allocate_money("100.00".parse().unwrap(), &rows).unwrap();
    assert_eq!(
        allocation
            .parts
            .iter()
            .map(|part| (
                part.id.as_str(),
                part.value.fixed_text(2).unwrap(),
                part.received_residual
            ))
            .collect::<Vec<_>>(),
        [
            ("a", "33.34".to_owned(), true),
            ("b", "33.33".to_owned(), false),
            ("c", "33.33".to_owned(), false),
        ]
    );
    assert_eq!(allocation, allocate_percentages(&rows).unwrap());
    let equal_positions = weights(&[("b", 1, "1"), ("a", 1, "1"), ("z", 0, "0")]);
    let allocation = allocate_money("0.01".parse().unwrap(), &equal_positions).unwrap();
    assert_eq!(
        allocation
            .parts
            .iter()
            .map(|part| (part.id.as_str(), part.value.to_string()))
            .collect::<Vec<_>>(),
        [("z", "0".into()), ("a", "0.01".into()), ("b", "0".into())]
    );
    let unequal = allocate_money(
        "100".parse().unwrap(),
        &weights(&[("a", 1, "1"), ("b", 2, "2"), ("c", 3, "3")]),
    )
    .unwrap();
    assert_eq!(
        unequal
            .parts
            .iter()
            .map(|p| p.value.fixed_text(2).unwrap())
            .collect::<Vec<_>>(),
        ["16.67", "33.33", "50.00"]
    );
}

#[test]
fn allocation_uses_exact_ratios_at_the_largest_money_boundary() {
    let total: Money = "999999999999999999.99".parse().unwrap();
    let result = allocate_money(
        total,
        &weights(&[
            ("a", 1, "99999999999999.999999"),
            ("b", 2, "99999999999999.999999"),
            ("c", 3, "99999999999999.999999"),
        ]),
    )
    .unwrap();
    assert_eq!(
        result
            .parts
            .iter()
            .map(|p| p.value.fixed_text(2).unwrap())
            .collect::<Vec<_>>(),
        [
            "333333333333333333.33",
            "333333333333333333.33",
            "333333333333333333.33"
        ]
    );
}

#[test]
fn allocation_validation_rejects_negative_or_undefined_bases() {
    assert_eq!(
        allocate_money("100".parse().unwrap(), &[]),
        Err(DecimalError::EmptyAllocation)
    );
    assert_eq!(
        allocate_money("100".parse().unwrap(), &weights(&[("a", 1, "0")])),
        Err(DecimalError::ZeroWeight)
    );
    assert_eq!(
        allocate_money("100".parse().unwrap(), &weights(&[("a", 1, "-1")])),
        Err(DecimalError::Negative)
    );
    assert_eq!(
        allocate_money("-100".parse().unwrap(), &weights(&[("a", 1, "1")])),
        Err(DecimalError::Negative)
    );
    assert_eq!(
        allocate_money(
            "100".parse().unwrap(),
            &weights(&[("a", 1, "1"), ("a", 2, "1")])
        ),
        Err(DecimalError::DuplicateIdentifier)
    );
    assert_eq!(
        allocate_money("100".parse().unwrap(), &weights(&[(" ", 1, "1")])),
        Err(DecimalError::InvalidIdentifier)
    );
    assert_eq!(
        allocate(decimal("0.001"), &weights(&[("a", 1, "1")]), 2),
        Err(DecimalError::Precision)
    );
    let zero = allocate_money("0".parse().unwrap(), &weights(&[("a", 1, "1")])).unwrap();
    assert_eq!(zero.parts[0].value, ExactDecimal::ZERO);
    assert!(!zero.parts[0].received_residual);
}

#[test]
fn allocation_is_independent_of_query_order_even_near_precision_limits() {
    let rows = weights(&[
        ("a", 1, "999999999999999999999999999"),
        ("b", 2, "0.00001"),
        ("c", 3, "0.99999"),
    ]);
    let result = allocate_money("100".parse().unwrap(), &rows).unwrap();
    for permutation in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let reordered = permutation.map(|i| rows[i].clone());
        assert_eq!(
            result,
            allocate_money("100".parse().unwrap(), &reordered).unwrap()
        );
    }
}

#[test]
fn generated_allocations_preserve_total_quota_and_determinism() {
    for total_cents in [0_i128, 1, 2, 7, 99, 100, 10001, 987654321] {
        for count in 1..=17 {
            let rows: Vec<_> = (0..count)
                .map(|i| AllocationWeight {
                    id: format!("item-{i:03}"),
                    position: i,
                    weight: decimal(&((i * 13 + 7) % 19 + 1).to_string()),
                })
                .collect();
            let total = Money::new(
                decimal(&total_cents.to_string())
                    .checked_div_exact(decimal("100"))
                    .unwrap(),
            )
            .unwrap();
            let output = allocate_money(total, &rows).unwrap();
            assert_eq!(
                output
                    .parts
                    .iter()
                    .map(|p| p.value.scaled_integer(2).unwrap())
                    .sum::<i128>(),
                total_cents
            );
            let sum: i128 = rows
                .iter()
                .map(|r| r.weight.scaled_integer(0).unwrap())
                .sum();
            for (part, row) in output.parts.iter().zip(&rows) {
                let numerator = total_cents * row.weight.scaled_integer(0).unwrap();
                let value = part.value.scaled_integer(2).unwrap();
                assert!(value == numerator / sum || value == numerator / sum + 1);
                assert_eq!(part.received_residual, value > numerator / sum);
            }
            let mut reversed = rows.clone();
            reversed.reverse();
            assert_eq!(output, allocate_money(total, &reversed).unwrap());
        }
    }
}

#[test]
fn cumulative_prices_are_monotone_and_telescope_for_fractional_quantities() {
    for price in ["0", "0.000001", "0.335", "1.234567", "99999.999999"] {
        let price: UnitPrice = price.parse().unwrap();
        let mut previous: Quantity = "0".parse().unwrap();
        let mut recognized: Money = "0".parse().unwrap();
        let mut sum = ExactDecimal::ZERO;
        for units in 1..=100 {
            let current = Quantity::new(
                decimal(&units.to_string())
                    .checked_div_exact(decimal("10"))
                    .unwrap(),
            )
            .unwrap();
            let result = unit_measurement_with_previous(
                "10".parse().unwrap(),
                previous,
                current,
                price,
                recognized,
            )
            .unwrap();
            assert!(result.current_accumulated >= recognized);
            assert!(!result.period_value.value().is_negative());
            sum = sum.checked_add(result.period_value.value()).unwrap();
            recognized = result.current_accumulated;
            previous = current;
        }
        assert_eq!(
            sum,
            decimal("10")
                .checked_mul(price.value())
                .unwrap()
                .truncate(2)
                .unwrap()
        );
        assert_eq!(recognized.value(), sum);
    }
}
