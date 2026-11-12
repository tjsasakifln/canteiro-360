//! Fictitious, deterministic presentation data. No persistence or authority is implied.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Company {
    #[default]
    Aurora,
    Horizonte,
}

impl Company {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "aurora" => Some(Self::Aurora),
            "horizonte" => Some(Self::Horizonte),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Aurora => "aurora",
            Self::Horizonte => "horizonte",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Aurora => "Construtora Aurora",
            Self::Horizonte => "Obras Horizonte",
        }
    }

    pub fn work(self) -> &'static str {
        match self {
            Self::Aurora => "Escola Vila Nova",
            Self::Horizonte => "Galpão Horizonte",
        }
    }

    pub fn contract(self) -> &'static str {
        match self {
            Self::Aurora => "CT-014/2026",
            Self::Horizonte => "CT-027/2026",
        }
    }

    pub fn version(self) -> &'static str {
        match self {
            Self::Aurora => "02",
            Self::Horizonte => "01",
        }
    }

    pub fn items(self) -> [BudgetItem; 3] {
        let quantities = match self {
            Self::Aurora => [600, 400, 800],
            Self::Horizonte => [450, 400, 600],
        };
        let prices = match self {
            Self::Aurora => [12_000, 18_000, 12_000],
            Self::Horizonte => [12_000, 13_500, 12_000],
        };
        [
            BudgetItem {
                code: "01",
                name: "Concreto para estrutura",
                unit: "m³",
                quantity: quantities[0],
                unit_cents: prices[0],
            },
            BudgetItem {
                code: "02",
                name: "Alvenaria de vedação",
                unit: "m²",
                quantity: quantities[1],
                unit_cents: prices[1],
            },
            BudgetItem {
                code: "03",
                name: "Revestimento de paredes",
                unit: "m²",
                quantity: quantities[2],
                unit_cents: prices[2],
            },
        ]
    }

    pub fn total_cents(self) -> u64 {
        self.items().iter().map(|item| item.total_cents()).sum()
    }
    pub fn gross_cents(self) -> u64 {
        match self {
            Self::Aurora => 1_920_000,
            Self::Horizonte => 1_540_000,
        }
    }
    pub fn retention_cents(self) -> u64 {
        match self {
            Self::Aurora => 100_000,
            Self::Horizonte => 50_000,
        }
    }
    pub fn net_cents(self) -> u64 {
        self.gross_cents() - self.retention_cents()
    }
    pub fn previous_cents(self) -> u64 {
        match self {
            Self::Aurora => 4_800_000,
            Self::Horizonte => 3_000_000,
        }
    }
    pub fn paid_cents(self) -> u64 {
        match self {
            Self::Aurora => 4_000_000,
            Self::Horizonte => 2_200_000,
        }
    }
    pub fn measured_cents(self) -> u64 {
        self.previous_cents() + self.gross_cents()
    }

    pub fn url(self, task: Task) -> String {
        format!("/prototipo/{}/{}", self.key(), task.key())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Dashboard,
    Budget,
    Measurement,
    Approvals,
    Documents,
}

impl Task {
    pub const ALL: [Self; 5] = [
        Self::Dashboard,
        Self::Budget,
        Self::Measurement,
        Self::Approvals,
        Self::Documents,
    ];
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|task| task.key() == value)
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Dashboard => "painel",
            Self::Budget => "orcamento",
            Self::Measurement => "medicao",
            Self::Approvals => "aprovacoes",
            Self::Documents => "documentos",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Painel da obra",
            Self::Budget => "Orçamento",
            Self::Measurement => "Preparar medição",
            Self::Approvals => "Aprovações",
            Self::Documents => "Documentos",
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Dashboard => "◫",
            Self::Budget => "≡",
            Self::Measurement => "▤",
            Self::Approvals => "✓",
            Self::Documents => "▱",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BudgetItem {
    pub code: &'static str,
    pub name: &'static str,
    pub unit: &'static str,
    pub quantity: u32,
    pub unit_cents: u64,
}

impl BudgetItem {
    pub fn total_cents(self) -> u64 {
        u64::from(self.quantity) * self.unit_cents
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemoState {
    pub company: Company,
    pub note: String,
    pub start: String,
    pub end: String,
    pub checked: bool,
    pub approved: bool,
}

impl DemoState {
    pub fn new(company: Company) -> Self {
        Self {
            company,
            note: String::new(),
            start: "2026-11-01".into(),
            end: "2026-11-15".into(),
            checked: false,
            approved: false,
        }
    }

    pub fn for_company(&self, company: Company) -> Self {
        if self.company == company {
            self.clone()
        } else {
            Self::new(company)
        }
    }

    pub fn has_edits(&self) -> bool {
        !self.note.is_empty()
            || self.start != "2026-11-01"
            || self.end != "2026-11-15"
            || self.approved
    }

    /// Native date controls follow the browser locale. This label confirms the
    /// actual ISO values in Portuguese without relying on that presentation.
    pub fn period_description(&self) -> String {
        match (portuguese_date(&self.start), portuguese_date(&self.end)) {
            (Some(start), Some(end)) => {
                let period = format!("Período selecionado: {start} a {end}.");
                if self.end < self.start {
                    format!("{period} A data final antecede a inicial; revise o período.")
                } else {
                    period
                }
            }
            (None, None) => {
                "Informe datas válidas de início e fim para conferir o período selecionado.".into()
            }
            (None, _) => {
                "Informe uma data inicial válida para conferir o período selecionado.".into()
            }
            (_, None) => {
                "Informe uma data final válida para conferir o período selecionado.".into()
            }
        }
    }

    pub fn validation(&self) -> Option<(&'static str, &'static str)> {
        if self.note.trim().chars().count() < 10 {
            return Some((
                "measurement-note",
                "Descreva a execução com pelo menos 10 caracteres. O texto digitado foi preservado.",
            ));
        }
        if !valid_date(&self.start) {
            return Some((
                "period-start",
                "Informe uma data inicial válida. Os demais campos foram preservados.",
            ));
        }
        if !valid_date(&self.end) {
            return Some((
                "period-end",
                "Informe uma data final válida. Os demais campos foram preservados.",
            ));
        }
        if self.end < self.start {
            return Some((
                "period-end",
                "A data final precisa ser igual ou posterior à inicial. Os campos foram preservados.",
            ));
        }
        None
    }
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return false;
    }
    let Ok(year) = value[..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = value[8..].parse::<u32>() else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    year > 0 && day > 0 && day <= days
}

fn portuguese_date(value: &str) -> Option<String> {
    if !valid_date(value) {
        return None;
    }
    let year = value[..4].parse::<u32>().ok()?;
    let month = value[5..7].parse::<usize>().ok()?;
    let day = value[8..].parse::<u32>().ok()?;
    let months = [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ];
    Some(format!("{day} de {} de {year}", months[month - 1]))
}

/// Format exact cents without binary floating point or locale-dependent parsing.
pub fn money(cents: u64) -> String {
    let whole = (cents / 100).to_string();
    let mut grouped = String::new();
    for (index, ch) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    format!("R$ {grouped},{:02}", cents % 100)
}

pub fn encode_query(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_context_never_exposes_draft_or_approval() {
        let mut state = DemoState::new(Company::Aurora);
        state.note = "Rascunho da empresa A".into();
        state.approved = true;
        assert_eq!(
            state.for_company(Company::Horizonte),
            DemoState::new(Company::Horizonte)
        );
        assert_eq!(
            state.for_company(Company::Aurora).note,
            "Rascunho da empresa A"
        );
    }

    #[test]
    fn fixtures_have_exact_distinct_totals_and_net_values() {
        assert_eq!(Company::Aurora.total_cents(), 24_000_000);
        assert_eq!(Company::Horizonte.total_cents(), 18_000_000);
        assert_eq!(Company::Aurora.net_cents(), 1_820_000);
        assert_eq!(Company::Horizonte.net_cents(), 1_490_000);
        assert_eq!(money(1_820_000), "R$ 18.200,00");
        assert_eq!(money(101), "R$ 1,01");
        assert_eq!(money(0), "R$ 0,00");
    }

    #[test]
    fn invalid_input_is_rejected_without_changing_the_draft() {
        let mut state = DemoState::new(Company::Aurora);
        state.note = "abc".into();
        let before = state.clone();
        assert_eq!(state.validation().unwrap().0, "measurement-note");
        assert_eq!(state, before);
        state.note = "Concreto executado no pavimento térreo".into();
        state.end = "2026-10-01".into();
        assert_eq!(state.validation().unwrap().0, "period-end");
        state.end = "2026-02-30".into();
        assert!(state.validation().is_some());
        state.end = "2026-11-30".into();
        assert!(state.validation().is_none());
        for invalid in ["+026-01-01", "2026-+1-05", "é12-03-04", "2026-02-29"] {
            assert!(!valid_date(invalid), "{invalid}");
        }
    }

    #[test]
    fn routes_and_queries_do_not_silently_choose_another_context() {
        assert_eq!(Company::parse("unknown"), None);
        assert_eq!(Task::parse("financeiro"), None);
        assert_eq!(encode_query("a&item=03 café"), "a%26item%3D03%20caf%C3%A9");
    }
}
