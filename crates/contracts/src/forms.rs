//! Reusable presentation contracts for the forms and paginated demonstration.
//!
//! Decimal text is interpreted explicitly as pt-BR. The demonstration is not a
//! persistence, permission or financial-command API. The complete calculation
//! engine remains a separate implementation step.

mod decimal;
mod measurement;

pub use decimal::{
    DecimalKind, FieldError, ParsedDate, ParsedDecimal, format_decimal_pt_br, parse_date_pt_br,
    parse_decimal_pt_br, parse_unit,
};
pub use measurement::{
    FormFieldError, FormPreview, MeasurementForm, MeasurementPage, MeasurementQuery,
    MeasurementRow, demo_export, demo_page, demo_row, preview_measurement,
    validate_measurement_form,
};
