use canteiro_contracts::forms::{
    FormPreview, MeasurementForm, MeasurementRow, format_decimal_pt_br,
};
use std::collections::{BTreeMap, BTreeSet};

/// Local presentation drafts, deliberately kept above route components.
/// This is not a persisted financial record or an offline business session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorksheetState {
    pub drafts: BTreeMap<u32, RowDraft>,
    pub editing: Option<u32>,
}

impl WorksheetState {
    pub fn has_edits(&self) -> bool {
        self.drafts.values().any(RowDraft::has_edits)
    }

    pub fn open(&mut self, row: MeasurementRow) {
        self.editing = Some(row.id);
        self.drafts
            .entry(row.id)
            .or_insert_with(|| RowDraft::new(row));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowDraft {
    pub original: MeasurementRow,
    pub initial: MeasurementForm,
    pub input: MeasurementForm,
    pub touched: BTreeSet<String>,
    pub submitted: bool,
    pub checked: Option<FormPreview>,
    pub checked_input: Option<MeasurementForm>,
    pub saved_input: Option<MeasurementForm>,
    pub saved_at: Option<String>,
    pub revision: u32,
}

impl RowDraft {
    pub fn new(original: MeasurementRow) -> Self {
        let input = MeasurementForm {
            row_id: original.id,
            current: format_decimal_pt_br(&original.current),
            unit_price: format_decimal_pt_br(&original.unit_price),
            percent: "0,00".into(),
            date: "12/11/2026".into(),
            unit: original.unit.clone(),
            notes: String::new(),
        };
        Self {
            original,
            initial: input.clone(),
            input,
            touched: BTreeSet::new(),
            submitted: false,
            checked: None,
            checked_input: None,
            saved_input: None,
            saved_at: None,
            revision: 0,
        }
    }

    pub fn has_edits(&self) -> bool {
        self.input != self.initial || self.saved_input.is_some()
    }

    pub fn is_saved(&self) -> bool {
        self.saved_input.as_ref() == Some(&self.input)
    }

    pub fn checked_for_current_input(&self) -> bool {
        self.checked_input.as_ref() == Some(&self.input)
            && self
                .checked
                .as_ref()
                .is_some_and(|result| result.errors.is_empty() && result.preview.is_some())
    }

    pub fn change(&mut self, field: &str, value: String) {
        match field {
            "current" => self.input.current = value,
            "unit_price" => self.input.unit_price = value,
            "percent" => self.input.percent = value,
            "date" => self.input.date = value,
            "unit" => self.input.unit = value,
            "notes" => self.input.notes = value,
            _ => return,
        }
        self.checked = None;
        self.checked_input = None;
    }

    pub fn value(&self, field: &str) -> String {
        match field {
            "current" => self.input.current.clone(),
            "unit_price" => self.input.unit_price.clone(),
            "percent" => self.input.percent.clone(),
            "date" => self.input.date.clone(),
            "unit" => self.input.unit.clone(),
            "notes" => self.input.notes.clone(),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prototype::model::{Company, DemoState};
    use canteiro_contracts::forms::{demo_row, validate_measurement_form};

    #[test]
    fn multiple_drafts_survive_reopening_and_are_never_carried_to_another_company() {
        let mut demo = DemoState::new(Company::Aurora);
        demo.worksheet.open(demo_row("aurora", 1).unwrap());
        assert!(!demo.has_edits());
        demo.worksheet
            .drafts
            .get_mut(&1)
            .unwrap()
            .change("notes", "Medição fictícia do pavimento térreo".into());
        demo.worksheet.open(demo_row("aurora", 2).unwrap());
        demo.worksheet
            .drafts
            .get_mut(&2)
            .unwrap()
            .change("current", "3".into());
        demo.worksheet.open(demo_row("aurora", 1).unwrap());
        assert_eq!(
            demo.worksheet.drafts[&1].input.notes,
            "Medição fictícia do pavimento térreo"
        );
        assert_eq!(demo.worksheet.drafts[&2].input.current, "3");
        assert!(demo.has_edits());
        let other = demo.for_company(Company::Horizonte);
        assert!(other.worksheet.drafts.is_empty());
        assert!(!other.has_edits());
        assert_eq!(demo.worksheet.drafts.len(), 2);
    }

    #[test]
    fn changing_one_field_invalidates_check_and_keeps_the_saved_version() {
        let mut draft = RowDraft::new(demo_row("aurora", 1).unwrap());
        draft.change("notes", "Justificativa fictícia preservada".into());
        draft.checked = Some(validate_measurement_form("aurora", &draft.input).unwrap());
        draft.checked_input = Some(draft.input.clone());
        draft.saved_input = Some(draft.input.clone());
        draft.saved_at = Some("2026-11-12T12:00:00.000Z".into());
        assert!(draft.checked_for_current_input());
        assert!(draft.is_saved());
        draft.change("unit_price", "1.234".into());
        assert!(!draft.checked_for_current_input());
        assert!(!draft.is_saved());
        assert_eq!(draft.input.notes, "Justificativa fictícia preservada");
        assert_eq!(draft.input.unit_price, "1.234");
        assert_eq!(draft.saved_input.as_ref().unwrap().unit_price, "10");
        assert!(draft.has_edits());
    }
}
