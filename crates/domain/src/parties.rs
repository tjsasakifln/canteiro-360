//! Identidades brasileiras e cadastros independentes de contas de acesso.
//! CNPJ: Receita Federal, Manual de Cálculo do DV, edição de 05/11/2024.
use crate::CivilDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyKind {
    Person,
    Company,
}
impl PartyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Company => "company",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Person => "Pessoa",
            Self::Company => "Empresa",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyRole {
    Customer,
    Supplier,
    Professional,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub name: String,
    pub email: String,
    pub phone: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Professional {
    pub council: String,
    pub registration: String,
    pub uf: String,
    pub valid_from: Option<CivilDate>,
    pub valid_until: Option<CivilDate>,
}
impl Professional {
    pub fn effective_on(&self, date: CivilDate) -> bool {
        !self.council.is_empty()
            && !self.registration.is_empty()
            && valid_uf(&self.uf)
            && self.valid_from.is_some_and(|start| start <= date)
            && self.valid_until.is_none_or(|end| date <= end)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PartyInput {
    pub kind: PartyKind,
    pub name: String,
    pub tax_id: Option<String>,
    pub roles: Vec<PartyRole>,
    #[serde(default)]
    pub contacts: Vec<Contact>,
    #[serde(default)]
    pub address: String,
    pub professional: Option<Professional>,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct PartyError {
    pub field: &'static str,
    pub message: &'static str,
}
fn invalid(field: &'static str, message: &'static str) -> PartyError {
    PartyError { field, message }
}

pub fn valid_uf(value: &str) -> bool {
    matches!(
        value,
        "AC" | "AL"
            | "AP"
            | "AM"
            | "BA"
            | "CE"
            | "DF"
            | "ES"
            | "GO"
            | "MA"
            | "MT"
            | "MS"
            | "MG"
            | "PA"
            | "PB"
            | "PR"
            | "PE"
            | "PI"
            | "RJ"
            | "RN"
            | "RS"
            | "RO"
            | "RR"
            | "SC"
            | "SP"
            | "SE"
            | "TO"
    )
}

fn plain(value: &str, maximum: usize, required: bool) -> bool {
    (!required || value.trim().chars().count() >= 2)
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

/// Aceita somente texto canônico ou máscara brasileira completa. Não remove
/// caracteres desconhecidos, não converte para número e mantém zeros iniciais.
pub fn normalize_tax_id(kind: PartyKind, input: &str) -> Result<String, PartyError> {
    let input = input.trim().to_ascii_uppercase();
    let (raw_len, masked_len, punctuation): (usize, usize, &[(usize, u8)]) = match kind {
        PartyKind::Person => (11, 14, &[(3, b'.'), (7, b'.'), (11, b'-')]),
        PartyKind::Company => (14, 18, &[(2, b'.'), (6, b'.'), (10, b'/'), (15, b'-')]),
    };
    if !input.is_ascii() || (input.len() != raw_len && input.len() != masked_len) {
        return Err(invalid(
            "tax_id",
            "Confira o CPF ou CNPJ completo, com ou sem a máscara brasileira.",
        ));
    }
    let raw = if input.len() == masked_len {
        if punctuation.iter().any(|(i, c)| input.as_bytes()[*i] != *c) {
            return Err(invalid(
                "tax_id",
                "A máscara do CPF ou CNPJ está incorreta.",
            ));
        }
        input
            .bytes()
            .enumerate()
            .filter(|(i, _)| !punctuation.iter().any(|(p, _)| p == i))
            .map(|(_, c)| c as char)
            .collect::<String>()
    } else {
        input
    };
    let bytes = raw.as_bytes();
    let base_len = raw_len - 2;
    if !bytes[base_len..].iter().all(u8::is_ascii_digit)
        || !bytes[..base_len]
            .iter()
            .all(|c| c.is_ascii_digit() || (kind == PartyKind::Company && c.is_ascii_uppercase()))
        || bytes.iter().all(|c| *c == bytes[0])
    {
        return Err(invalid(
            "tax_id",
            "O CPF ou CNPJ contém caracteres ou dígitos verificadores inválidos.",
        ));
    }
    for end in [base_len, base_len + 1] {
        let sum: u32 = bytes[..end]
            .iter()
            .rev()
            .enumerate()
            .map(|(i, c)| {
                let weight = if kind == PartyKind::Person {
                    i as u32 + 2
                } else {
                    i as u32 % 8 + 2
                };
                u32::from(*c - b'0') * weight
            })
            .sum();
        let remainder = sum % 11;
        let digit = if remainder < 2 { 0 } else { 11 - remainder };
        if bytes[end] - b'0' != digit as u8 {
            return Err(invalid(
                "tax_id",
                "Os dígitos verificadores não conferem. Revise o CPF ou CNPJ informado.",
            ));
        }
    }
    Ok(raw)
}

impl PartyInput {
    pub fn validated(mut self) -> Result<Self, PartyError> {
        self.name = self.name.trim().to_owned();
        self.address = self.address.trim().to_owned();
        if !plain(&self.name, 160, true) {
            return Err(invalid("name", "Informe o nome com 2 a 160 caracteres."));
        }
        if !plain(&self.address, 500, false) {
            return Err(invalid(
                "address",
                "Informe um endereço com até 500 caracteres, sem quebras de linha.",
            ));
        }
        self.tax_id = self
            .tax_id
            .filter(|s| !s.trim().is_empty())
            .map(|s| normalize_tax_id(self.kind, &s))
            .transpose()?;
        let roles: BTreeSet<_> = self.roles.iter().copied().collect();
        if roles.is_empty() || roles.len() != self.roles.len() {
            return Err(invalid(
                "roles",
                "Selecione ao menos uma função, sem repetições.",
            ));
        }
        self.roles = roles.into_iter().collect();
        if self.contacts.len() > 10 {
            return Err(invalid(
                "contacts",
                "Mantenha até dez contatos por cadastro.",
            ));
        }
        for contact in &mut self.contacts {
            contact.name = contact.name.trim().to_owned();
            contact.email = contact.email.trim().to_ascii_lowercase();
            contact.phone = contact.phone.trim().to_owned();
            if !plain(&contact.name, 160, true)
                || !plain(&contact.email, 254, false)
                || !plain(&contact.phone, 40, false)
            {
                return Err(invalid(
                    "contacts",
                    "Confira nome, e-mail e telefone dos contatos.",
                ));
            }
            if !contact.email.is_empty()
                && (!contact.email.contains('@') || contact.email.contains(char::is_whitespace))
            {
                return Err(invalid(
                    "contacts",
                    "Informe um e-mail válido ou deixe o contato sem e-mail.",
                ));
            }
        }
        if self.roles.contains(&PartyRole::Professional) != self.professional.is_some()
            || (self.professional.is_some() && self.kind != PartyKind::Person)
        {
            return Err(invalid(
                "professional",
                "O responsável profissional deve ser uma pessoa e ter seu registro separado da conta de acesso.",
            ));
        }
        if let Some(profile) = &mut self.professional {
            profile.council = profile.council.trim().to_ascii_uppercase();
            profile.registration = profile.registration.trim().to_owned();
            profile.uf = profile.uf.trim().to_ascii_uppercase();
            if !plain(&profile.council, 30, false)
                || !plain(&profile.registration, 60, false)
                || (!profile.uf.is_empty() && !valid_uf(&profile.uf))
            {
                return Err(invalid(
                    "professional",
                    "Confira conselho, registro e UF do profissional.",
                ));
            }
            if let (Some(start), Some(end)) = (profile.valid_from, profile.valid_until)
                && end < start
            {
                return Err(invalid(
                    "valid_until",
                    "O fim da atuação não pode anteceder o início.",
                ));
            }
            if profile.valid_until.is_some() && profile.valid_from.is_none() {
                return Err(invalid(
                    "valid_from",
                    "Informe o início da atuação antes de definir seu término.",
                ));
            }
        }
        Ok(self)
    }
}

/// O histórico bancário usa cópias imutáveis, nunca uma referência ao valor atual.
#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BankAccount {
    pub bank: String,
    pub branch: String,
    pub account: String,
    pub holder: String,
    pub holder_tax_id: String,
}
impl BankAccount {
    pub fn validated(mut self) -> Result<Self, PartyError> {
        self.bank = self.bank.trim().to_owned();
        self.branch = self.branch.trim().to_owned();
        self.account = self.account.trim().to_owned();
        self.holder = self.holder.trim().to_owned();
        if !plain(&self.bank, 80, true)
            || !plain(&self.branch, 30, true)
            || !plain(&self.account, 40, true)
            || !plain(&self.holder, 160, true)
        {
            return Err(invalid(
                "bank",
                "Informe banco, agência, conta e titular completos.",
            ));
        }
        self.holder_tax_id = normalize_tax_id(PartyKind::Person, &self.holder_tax_id)
            .or_else(|_| normalize_tax_id(PartyKind::Company, &self.holder_tax_id))?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receita_alphanumeric_reference_and_numeric_are_textual() {
        assert_eq!(
            normalize_tax_id(PartyKind::Company, "12.ABC.345/01DE-35").unwrap(),
            "12ABC34501DE35"
        );
        assert_eq!(
            normalize_tax_id(PartyKind::Company, "12.345.678/0001-95").unwrap(),
            "12345678000195"
        );
        assert_eq!(
            normalize_tax_id(PartyKind::Company, "12abc34501de35").unwrap(),
            "12ABC34501DE35"
        );
        assert_eq!(
            normalize_tax_id(PartyKind::Person, "529.982.247-25").unwrap(),
            "52998224725"
        );
    }
    #[test]
    fn malformed_masks_unicode_wrong_kind_and_check_digits_fail() {
        for value in [
            "12ABC34501DE34",
            "12ABC34501DE3A",
            "00000000000000",
            "12,ABC.345/01DE-35",
            "１２ABC34501DE35",
            "12ABC34501DE35x",
        ] {
            assert!(
                normalize_tax_id(PartyKind::Company, value).is_err(),
                "{value}"
            );
        }
        assert!(normalize_tax_id(PartyKind::Person, "12ABC34501DE35").is_err());
        assert!(normalize_tax_id(PartyKind::Person, "11111111111").is_err());
    }
    #[test]
    fn professional_has_no_implicit_login_and_expiry_is_inclusive() {
        let p = Professional {
            council: "CREA".into(),
            registration: "123456".into(),
            uf: "SC".into(),
            valid_from: Some("2026-01-01".parse().unwrap()),
            valid_until: Some("2026-11-19".parse().unwrap()),
        };
        assert!(p.effective_on("2026-11-19".parse().unwrap()));
        assert!(!p.effective_on("2026-11-20".parse().unwrap()));
    }
}
