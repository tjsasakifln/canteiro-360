use canteiro_domain::{
    CivilDate, UtcInstant,
    calendar::{
        Cadence, CalendarError, CalendarPolicy, PauseInterval, TimeZoneId, advance,
        civil_months_after, days_between, execution_end, execution_window, local_to_utc,
        pause_days, project_execution, schedule, utc_to_local, validate_pauses,
    },
};

fn date(value: &str) -> CivilDate {
    value.parse().unwrap()
}

fn pause(front: &str, start: &str, restart: Option<&str>) -> PauseInterval {
    PauseInterval {
        front: front.to_owned(),
        start: date(start),
        restart: restart.map(date),
    }
}

#[test]
fn original_anchor_survives_short_months_and_leap_years() {
    for (anchor, months, expected) in [
        ("2027-01-31", 1, "2027-02-28"),
        ("2027-01-31", 2, "2027-03-31"),
        ("2028-01-31", 1, "2028-02-29"),
        ("2028-01-31", 2, "2028-03-31"),
        ("2027-01-30", 1, "2027-02-28"),
        ("2027-01-30", 2, "2027-03-30"),
        ("2028-02-29", 12, "2029-02-28"),
        ("2028-02-29", 48, "2032-02-29"),
        ("1899-12-31", 2, "1900-02-28"),
        ("1999-12-31", 2, "2000-02-29"),
        ("2027-12-31", 2, "2028-02-29"),
    ] {
        assert_eq!(
            civil_months_after(date(anchor), months).unwrap(),
            date(expected)
        );
    }
    assert_eq!(
        advance(
            date("2028-02-29"),
            CalendarPolicy::Civil,
            Cadence::Annual,
            4
        )
        .unwrap(),
        date("2032-02-29")
    );
}

#[test]
fn fixed_and_civil_cadences_are_explicit_and_do_not_share_month_lengths() {
    let start = date("2027-01-31");
    for (cadence, fixed_days, civil_end) in [
        (Cadence::Weekly, 7, "2027-02-07"),
        (Cadence::Fortnightly, 15, "2027-02-15"),
        (Cadence::Monthly, 30, "2027-02-28"),
        (Cadence::Bimonthly, 60, "2027-03-31"),
        (Cadence::Quarterly, 90, "2027-04-30"),
        (Cadence::Semiannual, 180, "2027-07-31"),
        (Cadence::Annual, 365, "2028-01-31"),
    ] {
        let fixed = advance(start, CalendarPolicy::FixedDays, cadence, 1).unwrap();
        assert_eq!(days_between(start, fixed).unwrap(), fixed_days);
        assert_eq!(
            advance(start, CalendarPolicy::Civil, cadence, 1).unwrap(),
            date(civil_end)
        );
        assert_eq!(
            advance(start, CalendarPolicy::Civil, cadence, 0).unwrap(),
            start
        );
    }
    assert_eq!(
        advance(start, CalendarPolicy::FixedDays, Cadence::Monthly, 1).unwrap(),
        date("2027-03-02")
    );
}

#[test]
fn schedule_preserves_formal_end_and_marks_only_actual_residuals() {
    let short = schedule(
        date("2027-01-31"),
        date("2027-02-28"),
        CalendarPolicy::FixedDays,
        Cadence::Monthly,
    )
    .unwrap();
    assert_eq!(short.len(), 1);
    assert_eq!(short[0].end, date("2027-02-28"));
    assert_eq!(short[0].days, 28);
    assert!(short[0].residual);

    let civil = schedule(
        date("2027-01-31"),
        date("2027-04-15"),
        CalendarPolicy::Civil,
        Cadence::Monthly,
    )
    .unwrap();
    assert_eq!(
        civil.iter().map(|p| p.end.to_string()).collect::<Vec<_>>(),
        ["2027-02-28", "2027-03-31", "2027-04-15"]
    );
    assert_eq!(
        civil.iter().map(|p| p.days).collect::<Vec<_>>(),
        [28, 31, 15]
    );
    assert_eq!(
        civil.iter().map(|p| p.residual).collect::<Vec<_>>(),
        [false, false, true]
    );
    for pair in civil.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
    assert!(
        schedule(
            date("2027-01-31"),
            date("2027-01-31"),
            CalendarPolicy::Civil,
            Cadence::Monthly
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        schedule(
            date("2027-02-01"),
            date("2027-01-31"),
            CalendarPolicy::Civil,
            Cadence::Monthly
        ),
        Err(CalendarError::ReversedInterval)
    );
}

#[test]
fn schedules_conserve_duration_and_advance_monotonically_for_both_policies() {
    // Metamorphic properties across different anchors, leap/non-leap years,
    // policies and cadences; expected totals come from the original interval.
    for anchor in ["1900-01-31", "2000-01-31", "2027-01-30", "2028-02-29"] {
        let start = date(anchor);
        for duration in [1, 7, 15, 28, 31, 365, 366, 731] {
            let end = execution_end(start, duration).unwrap();
            for policy in [CalendarPolicy::Civil, CalendarPolicy::FixedDays] {
                for cadence in [
                    Cadence::Weekly,
                    Cadence::Monthly,
                    Cadence::Quarterly,
                    Cadence::Annual,
                ] {
                    let periods = schedule(start, end, policy, cadence).unwrap();
                    assert_eq!(periods.first().unwrap().start, start);
                    assert_eq!(periods.last().unwrap().end, end);
                    assert_eq!(periods.iter().map(|p| p.days).sum::<u32>(), duration);
                    assert!(periods.iter().all(|p| p.start < p.end && p.days > 0));
                    assert!(periods[..periods.len() - 1].iter().all(|p| !p.residual));
                    for pair in periods.windows(2) {
                        assert_eq!(pair[0].end, pair[1].start);
                    }
                }
            }
        }
    }
}

#[test]
fn execution_excludes_start_day_and_compares_both_ends_with_validity() {
    let valid = execution_window(
        date("2027-03-01"),
        10,
        date("2027-03-01"),
        date("2027-03-11"),
    )
    .unwrap();
    assert_eq!(valid.end, date("2027-03-11"));
    assert!(valid.within_validity);
    let late = execution_window(
        date("2027-05-30"),
        10,
        date("2027-03-01"),
        date("2027-05-31"),
    )
    .unwrap();
    assert_eq!(late.end, date("2027-06-09"));
    assert!(!late.within_validity);
    let early = execution_window(
        date("2027-02-28"),
        10,
        date("2027-03-01"),
        date("2027-03-31"),
    )
    .unwrap();
    assert!(!early.within_validity);
    assert_eq!(
        execution_window(
            date("2027-03-01"),
            10,
            date("2027-04-01"),
            date("2027-03-31")
        ),
        Err(CalendarError::ReversedInterval)
    );
}

#[test]
fn out_of_range_operations_fail_and_final_residual_never_exceeds_year_9999() {
    assert_eq!(
        execution_end(date("9999-12-31"), 1),
        Err(CalendarError::DateRange)
    );
    assert_eq!(
        execution_end(date("2027-01-01"), u32::MAX),
        Err(CalendarError::DateRange)
    );
    assert_eq!(
        civil_months_after(date("9999-12-01"), 1),
        Err(CalendarError::DateRange)
    );
    assert_eq!(
        advance(
            date("2027-01-01"),
            CalendarPolicy::Civil,
            Cadence::Annual,
            u32::MAX
        ),
        Err(CalendarError::DateRange)
    );
    assert_eq!(
        advance(
            date("2027-01-01"),
            CalendarPolicy::FixedDays,
            Cadence::Annual,
            u32::MAX
        ),
        Err(CalendarError::DateRange)
    );
    assert_eq!(
        days_between(date("2027-01-02"), date("2027-01-01")),
        Err(CalendarError::ReversedInterval)
    );
    for policy in [CalendarPolicy::Civil, CalendarPolicy::FixedDays] {
        let periods = schedule(
            date("9999-12-15"),
            date("9999-12-31"),
            policy,
            Cadence::Monthly,
        )
        .unwrap();
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].days, 16);
        assert!(periods[0].residual);
    }
    assert_eq!(
        execution_end(date("0001-01-01"), 0).unwrap(),
        date("0001-01-01")
    );
}

#[test]
fn pauses_are_half_open_and_scoped_to_one_front() {
    let pauses = [
        pause("estrutura", "2027-03-10", Some("2027-03-13")),
        pause("estrutura", "2027-03-13", Some("2027-03-15")),
        pause("instalacoes", "2027-03-10", Some("2027-03-20")),
        pause("estrutura", "2027-03-11", Some("2027-03-11")),
    ];
    let sum = pause_days(&pauses, "estrutura", None).unwrap();
    assert_eq!(sum.closed_days, 5);
    assert_eq!(sum.total_days, 5);
    assert!(!sum.has_open_pause);
    assert_eq!(
        pause_days(&pauses, "instalacoes", None).unwrap().total_days,
        10
    );
    assert_eq!(
        pause_days(&pauses, "acabamento", None).unwrap().total_days,
        0
    );
    let reordered: Vec<_> = pauses.iter().rev().cloned().collect();
    assert_eq!(pause_days(&reordered, "estrutura", None).unwrap(), sum);
    assert_eq!(
        pause_days(
            &[pause("estrutura", "2027-03-10", Some("2027-03-13"))],
            "estrutura",
            None
        )
        .unwrap()
        .total_days,
        3
    );
}

#[test]
fn overlap_open_overlap_and_reverse_pause_are_rejected_before_counting() {
    let first = pause("A", "2027-03-10", Some("2027-03-13"));
    for other in [
        pause("A", "2027-03-12", Some("2027-03-14")),
        pause("A", "2027-03-10", Some("2027-03-13")),
        pause("A", "2027-03-09", None),
        pause("A", "2027-03-12", None),
    ] {
        assert_eq!(
            validate_pauses(&[first.clone(), other]),
            Err(CalendarError::OverlappingPauses)
        );
    }
    assert_eq!(
        validate_pauses(&[pause("A", "2027-03-13", Some("2027-03-10"))]),
        Err(CalendarError::ReversedInterval)
    );
    for front in ["", " A", "A ", "A\nB"] {
        assert_eq!(
            pause_days(&[], front, None),
            Err(CalendarError::InvalidFront)
        );
    }
    // Invalid data in another front cannot be silently accepted as a valid set.
    assert_eq!(
        pause_days(
            &[first, pause("B", "2027-03-13", Some("2027-03-10"))],
            "A",
            None
        ),
        Err(CalendarError::ReversedInterval)
    );
}

#[test]
fn open_pauses_are_explicit_projections_and_do_not_rewrite_formal_dates() {
    let pauses = [
        pause("estrutura", "2027-03-10", Some("2027-03-13")),
        pause("estrutura", "2027-03-20", None),
    ];
    assert_eq!(
        pause_days(&pauses, "estrutura", None),
        Err(CalendarError::OpenPauseNeedsReference)
    );
    assert_eq!(
        pause_days(&pauses, "estrutura", Some(date("2027-03-19"))),
        Err(CalendarError::ReferenceBeforePause)
    );
    let sum = pause_days(&pauses, "estrutura", Some(date("2027-03-23"))).unwrap();
    assert_eq!(sum.closed_days, 3);
    assert_eq!(sum.open_days, 3);
    assert_eq!(sum.total_days, 6);
    assert!(sum.has_open_pause);
    assert_eq!(sum.as_of, Some(date("2027-03-23")));
    let projected = project_execution(
        date("2027-05-30"),
        date("2027-05-31"),
        "estrutura",
        &pauses,
        Some(date("2027-03-23")),
    )
    .unwrap();
    assert_eq!(projected.formal_end, date("2027-05-30"));
    assert_eq!(projected.valid_to, date("2027-05-31"));
    assert_eq!(projected.projected_end, date("2027-06-05"));
    assert!(!projected.within_validity);
    assert!(projected.provisional);
    // A different front receives no extension from this partial pause.
    let other = project_execution(
        date("2027-05-30"),
        date("2027-05-31"),
        "instalacoes",
        &pauses,
        None,
    )
    .unwrap();
    assert_eq!(other.projected_end, other.formal_end);
    assert!(other.within_validity);
    assert!(!other.provisional);
}

#[test]
fn canonical_policies_and_validated_timezones_serialize_as_text() {
    assert_eq!(
        serde_json::to_string(&CalendarPolicy::Civil).unwrap(),
        "\"CALENDARIO_CIVIL\""
    );
    assert_eq!(
        serde_json::to_string(&CalendarPolicy::FixedDays).unwrap(),
        "\"DIAS_FIXOS_SICOP\""
    );
    assert!(serde_json::from_str::<CalendarPolicy>("\"AUTOMATICO\"").is_err());
    let zone: TimeZoneId = "America/Sao_Paulo".parse().unwrap();
    assert_eq!(
        serde_json::to_string(&zone).unwrap(),
        "\"America/Sao_Paulo\""
    );
    assert_eq!(
        serde_json::from_str::<TimeZoneId>("\"America/Sao_Paulo\"").unwrap(),
        zone
    );
    for zone in ["", "America/Inexistente", " America/Sao_Paulo", "-03:00"] {
        assert_eq!(
            zone.parse::<TimeZoneId>(),
            Err(CalendarError::InvalidTimeZone)
        );
    }
    assert!(serde_json::from_str::<TimeZoneId>("-3").is_err());
    assert!(
        serde_json::from_str::<PauseInterval>(
            "{\"front\":\"A\",\"start\":\"2027-02-31\",\"restart\":null}"
        )
        .is_err()
    );
}

#[test]
fn regional_timezone_changes_dates_only_for_instants_not_civil_terms() {
    let zone: TimeZoneId = "America/Sao_Paulo".parse().unwrap();
    let instant: UtcInstant = "2027-01-01T01:30:00Z".parse().unwrap();
    assert_eq!(
        utc_to_local(instant, zone).unwrap(),
        "2026-12-31T22:30:00.000000-03:00"
    );
    assert_eq!(local_to_utc("2026-12-31T22:30:00", zone).unwrap(), instant);
    assert_eq!(
        local_to_utc("2026-12-31T22:30:00.123456", zone)
            .unwrap()
            .to_string(),
        "2027-01-01T01:30:00.123456Z"
    );
    assert_eq!(date("2027-01-01").to_string(), "2027-01-01");
    // São Paulo had daylight saving time in January 2018, but not January 2027.
    assert_eq!(
        local_to_utc("2018-01-15T12:00:00", zone)
            .unwrap()
            .to_string(),
        "2018-01-15T14:00:00.000000Z"
    );
    assert_eq!(
        local_to_utc("2027-01-15T12:00:00", zone)
            .unwrap()
            .to_string(),
        "2027-01-15T15:00:00.000000Z"
    );
}

#[test]
fn daylight_saving_gaps_and_folds_never_guess_a_timestamp() {
    let sao_paulo = "America/Sao_Paulo".parse().unwrap();
    assert_eq!(
        local_to_utc("2018-11-04T00:30:00", sao_paulo),
        Err(CalendarError::NonexistentLocalDateTime)
    );
    let new_york = "America/New_York".parse().unwrap();
    assert_eq!(
        local_to_utc("2027-11-07T01:30:00", new_york),
        Err(CalendarError::AmbiguousLocalDateTime)
    );
    assert_eq!(
        local_to_utc("2027-03-14T02:30:00", new_york),
        Err(CalendarError::NonexistentLocalDateTime)
    );
    let first: UtcInstant = "2027-11-07T01:30:00-04:00".parse().unwrap();
    let second: UtcInstant = "2027-11-07T01:30:00-05:00".parse().unwrap();
    assert_ne!(first, second);
    assert_eq!(
        utc_to_local(first, new_york).unwrap(),
        "2027-11-07T01:30:00.000000-04:00"
    );
    assert_eq!(
        utc_to_local(second, new_york).unwrap(),
        "2027-11-07T01:30:00.000000-05:00"
    );
}

#[test]
fn historical_subminute_offsets_never_shift_the_instant_when_formatted() {
    let instant: UtcInstant = "1880-01-01T12:00:00Z".parse().unwrap();
    for zone in ["America/Sao_Paulo", "Asia/Tokyo", "Europe/Paris"] {
        assert_eq!(
            utc_to_local(instant, zone.parse().unwrap()),
            Err(CalendarError::SubminuteOffset),
            "{zone}"
        );
    }
    // The instant remains exact and usable; only a misleading RFC3339
    // representation of the historical offset is refused.
    assert_eq!(
        utc_to_local(instant, "UTC".parse().unwrap()).unwrap(),
        "1880-01-01T12:00:00.000000+00:00"
    );
    assert_eq!(
        local_to_utc("1880-01-01T08:53:32", "America/Sao_Paulo".parse().unwrap()).unwrap(),
        instant
    );
}

#[test]
fn local_datetime_parser_rejects_implicit_precision_loss_and_ambiguous_forms() {
    let zone = "UTC".parse().unwrap();
    for value in [
        "2027-01-01",
        "01/01/2027T12:00:00",
        "2027-01-01 12:00:00",
        "2027-01-01T12:00:00Z",
        "2027-01-01T12:00:00-03:00",
        "2027-01-01T12:00:00.1234567",
        "2027-01-01T12:00:00.",
        "2027-01-01T24:00:00",
        "2027-01-01T12:00:60",
        "2027-02-29T12:00:00",
        "0000-01-01T12:00:00",
        "10000-01-01T12:00:00",
        "2027-01-01T+1:00:00",
        "2027-01-01T12:00:00.1e3",
        "2027-01-01T12:00:0é",
    ] {
        assert_eq!(
            local_to_utc(value, zone),
            Err(CalendarError::InvalidLocalDateTime),
            "{value}"
        );
    }
    assert_eq!(
        local_to_utc("2027-01-01T12:00:00.1", zone)
            .unwrap()
            .to_string(),
        "2027-01-01T12:00:00.100000Z"
    );
    let tokyo = "Asia/Tokyo".parse().unwrap();
    assert_eq!(
        local_to_utc("0001-01-01T00:00:00", tokyo),
        Err(CalendarError::DateRange)
    );
    let late: UtcInstant = "9999-12-31T23:59:59Z".parse().unwrap();
    assert_eq!(utc_to_local(late, tokyo), Err(CalendarError::DateRange));
}
