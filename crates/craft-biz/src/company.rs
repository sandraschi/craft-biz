//! One company dossier for the back-office crafts.
//!
//! SetupCraft is the only writer. Every other app reads the same file.
//! A demo dossier is labelled and is not written unless the caller accepts it.

use crate::at::{validate_iban, validate_uid};
use crate::ekr::seed_accounts;
use crate::{BizError, Result};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const BANK_ACCOUNT: u32 = 2800;
pub const CAPITAL_ACCOUNT: u32 = 9000;
pub const FULL_SUBSCRIBED_CENTS: i64 = 3_500_000;
pub const FULL_MIN_PAID_CENTS: i64 = 1_750_000;
pub const PRIVILEGED_SUBSCRIBED_CENTS: i64 = 1_000_000;
pub const PRIVILEGED_MIN_PAID_CENTS: i64 = 500_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    Demo,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapitalMode {
    /// Stammkapital EUR 35,000, at least EUR 17,500 paid in.
    Full,
    /// GmbHG section 10b: at least EUR 10,000 raised, at least EUR 5,000 paid in, for ten years.
    Privileged,
    /// FlexKapG: at least EUR 10,000 subscribed, at least EUR 5,000 paid in, as the permanent minimum.
    Flex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Shareholder,
    ManagingDirector,
    Employee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonSource {
    Onboarding,
    Hirecraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmploymentKind {
    Angestellt,
    Geringfuegig,
    Lehre,
    DirectorWage,
    DirectorNoWage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalRole {
    Bookkeeping,
    Advisor,
    Notary,
    Lawyer,
    Bank,
    Insurance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dossier {
    pub schema_version: u32,
    pub vendor: Vendor,
    /// The founder sets this after reading `advice`. A live company is not written before that.
    pub preparation: Preparation,
    pub company: Company,
    pub capital: Capital,
    pub bank: Bank,
    pub people: Vec<Person>,
    pub externals: Vec<External>,
    pub photos: Photos,
    pub machine: Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    pub name: String,
    pub legal_form: String,
    pub street: String,
    pub plz: String,
    pub city: String,
    pub court: String,
    pub fnr: String,
    pub uid: String,
    pub oenace: String,
    pub finanzamt: String,
    pub fiscal_year_start: NaiveDate,
    pub founding_date: NaiveDate,
    pub wko_fachgruppe: String,
    pub email: String,
    pub phone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capital {
    pub mode: CapitalMode,
    pub subscribed_cents: i64,
    pub paid_in_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bank {
    pub iban: String,
    pub name: String,
    /// When set, the paid-in capital is already in this export. No opening lines.
    pub george_csv: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub code: String,
    pub roles: Vec<Role>,
    pub source: PersonSource,
    pub asvg_wage: bool,
    pub legal_name: String,
    pub birth_date: NaiveDate,
    pub svnr: Option<String>,
    pub address: String,
    pub email: String,
    pub phone: String,
    pub photo_file: Option<String>,
    pub hire_date: Option<NaiveDate>,
    pub position: Option<String>,
    pub department: Option<String>,
    pub work_text: Option<String>,
    pub weekly_minutes: Option<i64>,
    pub kv_name: Option<String>,
    pub kv_group: Option<String>,
    pub employment_kind: Option<EmploymentKind>,
    pub monthly_gross_cents: Option<i64>,
    pub iban: Option<String>,
    pub sonderzahlungen: bool,
    pub shares: Option<u32>,
    pub drawing_rights: Option<String>,
    pub candidate_id: Option<String>,
    pub probation_end: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct External {
    pub role: ExternalRole,
    pub firm: String,
    pub contact: String,
    pub email: String,
    pub phone: String,
    pub mandate_start: NaiveDate,
    pub data_shared: Vec<String>,
    pub avv_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Photos {
    pub office: Vec<String>,
    pub group: Option<String>,
    pub logo: Option<String>,
    pub portraits: Vec<Portrait>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Portrait {
    pub person_code: String,
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    pub tag: String,
    pub name: String,
    pub model: String,
    pub serial: String,
    pub holder_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpeningLine {
    pub account: u32,
    pub name: String,
    pub debit_cents: i64,
    pub credit_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preparation {
    pub acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opening {
    pub lines: Vec<OpeningLine>,
    pub outstanding_cents: i64,
    /// Set for a gruendungsprivilegierte GmbH: ten years after the founding date.
    pub privilege_ends: Option<NaiveDate>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    pub relative_path: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
    pub fingerprint: String,
    pub changed: Vec<String>,
    pub unchanged: Vec<String>,
}

/// Austrian SVNR: 10 digits, check digit in position 4, birth DDMMYY after it.
pub fn validate_svnr(input: &str) -> Result<String> {
    let s: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() != 10 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BizError::Invalid(format!(
            "SVNR must be 10 digits: {input:?}"
        )));
    }
    let digits: Vec<u32> = s.bytes().map(|b| u32::from(b - b'0')).collect();
    let weights = [3u32, 7, 9, 0, 5, 8, 4, 2, 1, 6];
    let sum: u32 = digits.iter().zip(weights).map(|(d, w)| d * w).sum();
    let check = sum % 11;
    match digits.get(3) {
        Some(&got) if check < 10 && got == check => Ok(s),
        _ => Err(BizError::Invalid(format!(
            "SVNR check digit wrong: {input:?}"
        ))),
    }
}

/// Shape only: 1-6 digits and one letter. This does not look the number up.
pub fn validate_fnr(input: &str) -> Result<String> {
    let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let rest = compact
        .strip_prefix("FN")
        .or_else(|| compact.strip_prefix("fn"))
        .unwrap_or(compact.as_str());
    let mut digits = String::new();
    let mut letter = None;
    for c in rest.chars() {
        if c.is_ascii_digit() && letter.is_none() {
            digits.push(c);
        } else if c.is_ascii_alphabetic() && letter.is_none() && !digits.is_empty() {
            letter = Some(c.to_ascii_lowercase());
        } else {
            return Err(BizError::Invalid(format!(
                "Firmenbuchnummer shape is digits plus one letter: {input:?}"
            )));
        }
    }
    let Some(letter) = letter else {
        return Err(BizError::Invalid(format!(
            "Firmenbuchnummer needs a check letter: {input:?}"
        )));
    };
    if !(1..=6).contains(&digits.len()) {
        return Err(BizError::Invalid(format!(
            "Firmenbuchnummer needs 1-6 digits: {input:?}"
        )));
    }
    Ok(format!("{digits}{letter}"))
}

pub fn validate_wien_plz(input: &str) -> Result<String> {
    if input.len() != 4 || !input.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BizError::Invalid(format!(
            "PLZ must be 4 digits: {input:?}"
        )));
    }
    let n: u16 = input
        .parse()
        .map_err(|_| BizError::Invalid(format!("PLZ must be 4 digits: {input:?}")))?;
    if (1010..=1230).contains(&n) {
        Ok(input.to_string())
    } else {
        Err(BizError::Invalid(format!(
            "Wien PLZ must be 1010-1230: {input:?}"
        )))
    }
}

pub fn is_team_member(person: &Person) -> bool {
    let director = person.roles.contains(&Role::ManagingDirector);
    let employee = person.roles.contains(&Role::Employee);
    employee || (director && person.asvg_wage)
}

pub fn card_gaps(person: &Person) -> Vec<&'static str> {
    if !is_team_member(person) {
        return Vec::new();
    }
    let mut gaps = Vec::new();
    if blank(person.photo_file.as_deref()) {
        gaps.push("photo");
    }
    if blank(person.svnr.as_deref()) {
        gaps.push("svnr");
    }
    if person.address.trim().is_empty() {
        gaps.push("address");
    }
    if person.hire_date.is_none() {
        gaps.push("hire_date");
    }
    if blank(person.position.as_deref()) {
        gaps.push("position");
    }
    if blank(person.work_text.as_deref()) {
        gaps.push("work_text");
    }
    gaps
}

pub fn validate(d: &Dossier) -> Result<()> {
    if d.schema_version != SCHEMA_VERSION {
        return Err(BizError::Invalid(format!(
            "dossier schema {} is not {SCHEMA_VERSION}",
            d.schema_version
        )));
    }
    require_text(&d.company.name, "company name", 200)?;
    require_text(&d.company.legal_form, "legal form", 40)?;
    require_text(&d.company.street, "street", 200)?;
    require_text(&d.company.city, "city", 80)?;
    require_text(&d.company.court, "court", 120)?;
    require_text(&d.company.oenace, "oenace", 16)?;
    require_text(&d.company.finanzamt, "finanzamt", 120)?;
    require_text(&d.company.wko_fachgruppe, "wko fachgruppe", 160)?;
    check_email(&d.company.email)?;
    require_text(&d.company.phone, "phone", 40)?;
    validate_seat(&d.company.city, &d.company.plz)?;
    check_legal_form(&d.company.legal_form)?;
    validate_fnr(&d.company.fnr)?;
    validate_uid(&d.company.uid)?;
    validate_iban(&d.bank.iban)?;
    require_text(&d.bank.name, "bank name", 120)?;
    check_capital(&d.company.legal_form, &d.capital)?;
    if d.people.is_empty() {
        return Err(BizError::Invalid(
            "the dossier needs at least one person".into(),
        ));
    }
    if !d
        .people
        .iter()
        .any(|p| p.roles.contains(&Role::Shareholder))
    {
        return Err(BizError::Invalid("the dossier needs a shareholder".into()));
    }
    let mut codes = Vec::new();
    for person in &d.people {
        let code = normalize_code(&person.code)?;
        if codes.contains(&code) {
            return Err(BizError::Invalid(format!("duplicate person code {code}")));
        }
        codes.push(code);
        check_person(person)?;
    }
    for external in &d.externals {
        check_external(external)?;
    }
    require_text(&d.machine.tag, "machine tag", 32)?;
    require_text(&d.machine.name, "machine name", 80)?;
    require_text(&d.machine.model, "machine model", 80)?;
    require_text(&d.machine.serial, "machine serial", 80)?;
    let holder = normalize_code(&d.machine.holder_code)?;
    if !d
        .people
        .iter()
        .any(|p| normalize_code(&p.code).ok().as_deref() == Some(holder.as_str()))
    {
        return Err(BizError::Invalid(
            "machine holder must be a person code".into(),
        ));
    }
    Ok(())
}

pub fn opening(d: &Dossier) -> Result<Opening> {
    validate(d)?;
    let outstanding = d
        .capital
        .subscribed_cents
        .checked_sub(d.capital.paid_in_cents)
        .ok_or(BizError::Overflow)?;
    let privilege_ends = privilege_end(d)?;
    let note = capital_note(d, privilege_ends);
    if d.bank.george_csv.is_some() {
        return Ok(Opening {
            lines: Vec::new(),
            outstanding_cents: outstanding,
            privilege_ends,
            note: format!(
                "Paid-in capital is already in the George export. No opening lines. {note}"
            ),
        });
    }
    let bank_name = account_name(BANK_ACCOUNT)?;
    let capital_name = account_name(CAPITAL_ACCOUNT)?;
    let paid = d.capital.paid_in_cents;
    Ok(Opening {
        lines: vec![
            OpeningLine {
                account: BANK_ACCOUNT,
                name: bank_name,
                debit_cents: paid,
                credit_cents: 0,
            },
            OpeningLine {
                account: CAPITAL_ACCOUNT,
                name: capital_name,
                debit_cents: 0,
                credit_cents: paid,
            },
        ],
        outstanding_cents: outstanding,
        privilege_ends,
        note,
    })
}

pub fn fingerprint(d: &Dossier) -> Result<String> {
    validate(d)?;
    let uid = validate_uid(&d.company.uid)?;
    let fnr = validate_fnr(&d.company.fnr)?;
    let iban = validate_iban(&d.bank.iban)?;
    let tail = iban
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let members: Vec<&Person> = team_members(d);
    let complete = members.iter().filter(|p| card_gaps(p).is_empty()).count();
    let vendor = match d.vendor {
        Vendor::Demo => "demo",
        Vendor::Live => "live",
    };
    Ok(format!(
        "fn={fnr}|uid={uid}|employees={}|iban_tail={tail}|cards_complete={complete}|vendor={vendor}",
        members.len()
    ))
}

pub fn package(d: &Dossier) -> Result<Vec<PackageFile>> {
    let fp = fingerprint(d)?;
    let open = opening(d)?;
    let mut files = vec![
        file("dossier.json", &to_pretty(&json_dossier(d)?)?),
        file("fingerprint.txt", &format!("{fp}\n")),
        file("buchcraft/opening.json", &to_pretty(&json_opening(&open)?)?),
        file("teamcraft/employees.json", &to_pretty(&json_employees(d)?)?),
        file("billcraft/seller.json", &to_pretty(&json_seller(d, &fp)?)?),
        file("contractcraft/drafts.json", &to_pretty(&json_drafts(d)?)?),
        file("taskcraft/tasks.json", &to_pretty(&json_tasks(d)?)?),
        file("assetcraft/machine.json", &to_pretty(&json_machine(d)?)?),
        file("wikicraft/handbook.md", &handbook(d)),
        file("impressum.txt", &impressum(d)?),
        file("advisors.json", &to_pretty(&json_advisors(d)?)?),
        file("deskcraft/profile.json", &to_pretty(&json_desk(d)?)?),
        file("gaincraft/vendor.json", &to_pretty(&json_gain(d)?)?),
        file("formcraft/contact.json", &to_pretty(&json_form(d)?)?),
        file("spendcraft/policy.json", &to_pretty(&json_spend()?)?),
        file("helmcraft/cockpit.json", &to_pretty(&json_helm(d, &fp)?)?),
        file("beraterpaket/index.txt", &berater_index(d)),
        file("advice/prepare.md", &crate::advice::markdown()),
    ];
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

pub fn write_package(dir: &Path, d: &Dossier, accept_demo: bool) -> Result<ApplyReport> {
    if d.vendor == Vendor::Live && !d.preparation.acknowledged {
        return Err(BizError::Invalid(
            "read setupcraft prepare, then acknowledge, then enter your own company".into(),
        ));
    }
    if d.vendor == Vendor::Demo && !accept_demo {
        return Err(BizError::Invalid(
            "demo dossier needs --accept-demo before it is written".into(),
        ));
    }
    let fp = fingerprint(d)?;
    let files = package(d)?;
    fs::create_dir_all(dir)?;
    let mut changed = Vec::new();
    let mut unchanged = Vec::new();
    for item in files {
        let path = dir.join(&item.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let same = fs::read(&path)
            .ok()
            .is_some_and(|bytes| bytes == item.body.as_bytes());
        if same {
            unchanged.push(item.relative_path);
        } else {
            fs::write(&path, item.body.as_bytes())?;
            changed.push(item.relative_path);
        }
    }
    Ok(ApplyReport {
        fingerprint: fp,
        changed,
        unchanged,
    })
}

pub fn load_dossier(path: &Path) -> Result<Dossier> {
    let text = fs::read_to_string(path)?;
    let dossier: Dossier = serde_json::from_str(&text)?;
    validate(&dossier)?;
    Ok(dossier)
}

pub fn load_unvalidated(path: &Path) -> Result<Dossier> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn save_dossier(path: &Path, dossier: &Dossier) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, to_pretty(&serde_json::to_value(dossier)?)?)?;
    Ok(())
}

pub fn acknowledge(dossier: &mut Dossier) {
    dossier.preparation.acknowledged = true;
}

/// A live skeleton. It does not validate, and it is not acknowledged.
pub fn startup_template() -> Dossier {
    let mut dossier = sandrasteve_demo();
    dossier.vendor = Vendor::Live;
    dossier.preparation.acknowledged = false;
    dossier.company.name = "Meine GmbH".into();
    dossier.company.city = "Graz".into();
    dossier.company.plz = "8010".into();
    dossier.company.court = "Landesgericht fuer ZRS Graz".into();
    dossier.company.uid = "ATU00000000".into();
    dossier.company.email = "office@example.com".into();
    dossier
}

/// Labelled demo for sandrasteve GmbH. The UID and IBAN are checksum fixtures.
pub fn sandrasteve_demo() -> Dossier {
    let founding = date(2026, 11, 16);
    Dossier {
        schema_version: SCHEMA_VERSION,
        vendor: Vendor::Demo,
        preparation: Preparation { acknowledged: true },
        company: Company {
            name: "sandrasteve GmbH".into(),
            legal_form: "GmbH".into(),
            street: "Mariahilfer Strasse 1".into(),
            plz: "1060".into(),
            city: "Wien".into(),
            court: "Handelsgericht Wien".into(),
            fnr: "123456a".into(),
            uid: "ATU13585627".into(),
            oenace: "62.01".into(),
            finanzamt: "Finanzamt Oesterreich".into(),
            fiscal_year_start: date(2026, 1, 1),
            founding_date: founding,
            wko_fachgruppe: "UBIT Informationstechnologie".into(),
            email: "office@sandrasteve.example".into(),
            phone: "+43 1 0000000".into(),
        },
        capital: Capital {
            mode: CapitalMode::Privileged,
            subscribed_cents: PRIVILEGED_SUBSCRIBED_CENTS,
            paid_in_cents: PRIVILEGED_MIN_PAID_CENTS,
        },
        bank: Bank {
            iban: "AT611904300234573201".into(),
            name: "Demo Bank".into(),
            george_csv: None,
        },
        people: vec![
            Person {
                code: "CEO".into(),
                roles: vec![Role::Shareholder, Role::ManagingDirector],
                source: PersonSource::Onboarding,
                asvg_wage: true,
                legal_name: "Alex Berger".into(),
                birth_date: date(1985, 4, 12),
                svnr: Some("1231120485".into()),
                address: "Neubaugasse 1, 1070 Wien".into(),
                email: "alex.berger@sandrasteve.example".into(),
                phone: "+43 1 0000001".into(),
                photo_file: Some("ceo.png".into()),
                hire_date: Some(founding),
                position: Some("Geschaeftsfuehrung".into()),
                department: Some("Leitung".into()),
                work_text: Some("Fuehrt sandrasteve GmbH.".into()),
                weekly_minutes: Some(2310),
                kv_name: Some("IT-KV".into()),
                kv_group: Some("ST1".into()),
                employment_kind: Some(EmploymentKind::DirectorWage),
                monthly_gross_cents: Some(450_000),
                iban: None,
                sonderzahlungen: true,
                shares: Some(100),
                drawing_rights: Some("selbständig".into()),
                candidate_id: None,
                probation_end: None,
            },
            Person {
                code: "DEV1".into(),
                roles: vec![Role::Employee],
                source: PersonSource::Onboarding,
                asvg_wage: true,
                legal_name: "Sam Hofer".into(),
                birth_date: date(1992, 9, 3),
                svnr: Some("4578030992".into()),
                address: "Lindengasse 2, 1070 Wien".into(),
                email: "sam.hofer@sandrasteve.example".into(),
                phone: "+43 1 0000002".into(),
                photo_file: Some("sam.png".into()),
                hire_date: Some(founding),
                position: Some("Entwickler".into()),
                department: Some("Produkt".into()),
                work_text: Some("Baut die Craft-Apps der Firma.".into()),
                weekly_minutes: Some(2310),
                kv_name: Some("IT-KV".into()),
                kv_group: Some("ST1".into()),
                employment_kind: Some(EmploymentKind::Angestellt),
                monthly_gross_cents: Some(320_000),
                iban: None,
                sonderzahlungen: true,
                shares: None,
                drawing_rights: None,
                candidate_id: None,
                probation_end: Some(date(2026, 12, 16)),
            },
        ],
        externals: vec![
            External {
                role: ExternalRole::Bookkeeping,
                firm: "Kanzlei Muster".into(),
                contact: "Eva Muster".into(),
                email: "buchhaltung@example.com".into(),
                phone: "+43 1 1111111".into(),
                mandate_start: date(2026, 11, 1),
                data_shared: vec!["books".into(), "payroll sheets".into()],
                avv_present: true,
            },
            External {
                role: ExternalRole::Advisor,
                firm: "Beratung Nord".into(),
                contact: "Jonas Nord".into(),
                email: "beratung@example.com".into(),
                phone: "+43 1 2222222".into(),
                mandate_start: date(2026, 11, 1),
                data_shared: vec!["company profile".into()],
                avv_present: false,
            },
        ],
        photos: Photos {
            office: vec!["office.jpg".into()],
            group: Some("team.jpg".into()),
            logo: Some("logo.png".into()),
            portraits: vec![
                Portrait {
                    person_code: "CEO".into(),
                    file: "ceo.png".into(),
                },
                Portrait {
                    person_code: "DEV1".into(),
                    file: "sam.png".into(),
                },
            ],
        },
        machine: Machine {
            tag: "PC-001".into(),
            name: "CEO PC".into(),
            model: "unspecified demo workstation".into(),
            serial: "DEMO-PC-1".into(),
            holder_code: "CEO".into(),
        },
    }
}

fn team_members(d: &Dossier) -> Vec<&Person> {
    let mut members: Vec<&Person> = d.people.iter().filter(|p| is_team_member(p)).collect();
    members.sort_by(|a, b| a.code.cmp(&b.code));
    members
}

fn check_capital(legal_form: &str, c: &Capital) -> Result<()> {
    let form = legal_kind(legal_form)?;
    let half = c
        .subscribed_cents
        .checked_div(2)
        .ok_or(BizError::Overflow)?;
    let fits = c.paid_in_cents <= c.subscribed_cents && c.paid_in_cents >= 0;
    let ok = match (form, c.mode) {
        (LegalKind::Gmbh, CapitalMode::Full) => {
            c.subscribed_cents >= FULL_SUBSCRIBED_CENTS
                && c.paid_in_cents >= FULL_MIN_PAID_CENTS
                && c.paid_in_cents >= half
                && fits
        }
        (LegalKind::Gmbh, CapitalMode::Privileged) => {
            c.subscribed_cents >= PRIVILEGED_SUBSCRIBED_CENTS
                && c.subscribed_cents < FULL_SUBSCRIBED_CENTS
                && c.paid_in_cents >= PRIVILEGED_MIN_PAID_CENTS
                && c.paid_in_cents >= half
                && fits
        }
        (LegalKind::Flex, CapitalMode::Flex) => {
            c.subscribed_cents >= PRIVILEGED_SUBSCRIBED_CENTS
                && c.paid_in_cents >= PRIVILEGED_MIN_PAID_CENTS
                && fits
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(BizError::Invalid(
            "capital does not match the legal form: GmbH is full (at least 35000 euro, at least half paid) or privileged (at least 10000 and below 35000, at least half and 5000 paid, for ten years); FlexKapG is flex (at least 10000 subscribed, at least 5000 paid)".into(),
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LegalKind {
    Gmbh,
    Flex,
}

fn legal_kind(form: &str) -> Result<LegalKind> {
    if form.eq_ignore_ascii_case("GmbH") {
        Ok(LegalKind::Gmbh)
    } else if form.eq_ignore_ascii_case("FlexKapG") || form.eq_ignore_ascii_case("FlexCo") {
        Ok(LegalKind::Flex)
    } else {
        Err(BizError::Invalid("legal form is GmbH or FlexKapG".into()))
    }
}

fn check_legal_form(form: &str) -> Result<()> {
    legal_kind(form).map(|_| ())
}

fn validate_seat(city: &str, plz: &str) -> Result<()> {
    if city.eq_ignore_ascii_case("Wien") {
        return validate_wien_plz(plz).map(|_| ());
    }
    if plz.len() != 4 || !plz.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BizError::Invalid(format!(
            "Austrian PLZ must be 4 digits: {plz:?}"
        )));
    }
    Ok(())
}

fn privilege_end(d: &Dossier) -> Result<Option<NaiveDate>> {
    if d.capital.mode != CapitalMode::Privileged {
        return Ok(None);
    }
    add_years(d.company.founding_date, 10).map(Some)
}

fn capital_note(d: &Dossier, privilege_ends: Option<NaiveDate>) -> String {
    let unpaid = "Unpaid capital is reported and not posted. The seed chart has no Ausstehende Einlagen account.";
    match (d.capital.mode, privilege_ends) {
        (CapitalMode::Privileged, Some(day)) => {
            format!("Gruendungsprivilegierung runs until {day}. {unpaid}")
        }
        (CapitalMode::Privileged, None) => format!("Gruendungsprivilegierung. {unpaid}"),
        (CapitalMode::Flex, _) => {
            format!("FlexKapG minimum capital is permanent, not a ten-year privilege. {unpaid}")
        }
        (CapitalMode::Full, _) => unpaid.to_string(),
    }
}

fn add_years(day: NaiveDate, years: i32) -> Result<NaiveDate> {
    let year = day.year().checked_add(years).ok_or(BizError::Overflow)?;
    NaiveDate::from_ymd_opt(year, day.month(), day.day())
        .or_else(|| NaiveDate::from_ymd_opt(year, day.month(), day.day().saturating_sub(1)))
        .ok_or(BizError::Overflow)
}

fn check_person(p: &Person) -> Result<()> {
    normalize_code(&p.code)?;
    require_text(&p.legal_name, "person name", 120)?;
    check_email(&p.email)?;
    require_text(&p.phone, "person phone", 40)?;
    if p.roles.is_empty() {
        return Err(BizError::Invalid(format!("{} needs a role", p.code)));
    }
    let mut seen = Vec::new();
    for role in &p.roles {
        if seen.contains(role) {
            return Err(BizError::Invalid(format!("{} repeats a role", p.code)));
        }
        seen.push(*role);
    }
    if p.roles.contains(&Role::Shareholder) {
        match p.shares {
            Some(n) if n > 0 => {}
            _ => {
                return Err(BizError::Invalid(format!(
                    "{} is a shareholder and needs a share count",
                    p.code
                )))
            }
        }
    }
    if let Some(svnr) = &p.svnr {
        let svnr = validate_svnr(svnr)?;
        if !svnr_matches_birth(&svnr, p.birth_date)? {
            return Err(BizError::Invalid(format!(
                "{} SVNR does not match the birth date",
                p.code
            )));
        }
    }
    if let Some(iban) = &p.iban {
        validate_iban(iban)?;
    }
    if let Some(gross) = p.monthly_gross_cents {
        if gross < 0 {
            return Err(BizError::Invalid(format!(
                "{} gross cannot be negative",
                p.code
            )));
        }
    }
    if let Some(minutes) = p.weekly_minutes {
        if !(1..=3600).contains(&minutes) {
            return Err(BizError::Invalid(format!(
                "{} weekly minutes must be 1-3600",
                p.code
            )));
        }
    }
    if is_team_member(p) {
        if p.weekly_minutes.is_none()
            || p.employment_kind.is_none()
            || p.monthly_gross_cents.is_none()
        {
            return Err(BizError::Invalid(format!(
                "{} needs hours, employment kind, and monthly gross",
                p.code
            )));
        }
        if p.svnr.is_none() {
            return Err(BizError::Invalid(format!("{} needs an SVNR", p.code)));
        }
        if p.source == PersonSource::Onboarding && !card_gaps(p).is_empty() {
            return Err(BizError::Invalid(format!(
                "{} founding card is missing {}",
                p.code,
                card_gaps(p).join(", ")
            )));
        }
        if p.source == PersonSource::Hirecraft && blank(p.candidate_id.as_deref()) {
            return Err(BizError::Invalid(format!(
                "{} came from hirecraft and needs a candidate id",
                p.code
            )));
        }
    }
    if p.roles.contains(&Role::ManagingDirector)
        && !p.asvg_wage
        && p.employment_kind
            .is_some_and(|k| k != EmploymentKind::DirectorNoWage)
    {
        return Err(BizError::Invalid(format!(
            "{} has no wage, so the employment kind is director_no_wage",
            p.code
        )));
    }
    Ok(())
}

fn check_external(e: &External) -> Result<()> {
    require_text(&e.firm, "advisor firm", 160)?;
    require_text(&e.contact, "advisor contact", 120)?;
    check_email(&e.email)?;
    require_text(&e.phone, "advisor phone", 40)?;
    if e.data_shared.is_empty() || e.data_shared.iter().any(|s| s.trim().is_empty()) {
        return Err(BizError::Invalid(format!(
            "{} needs the list of data they may see",
            e.firm
        )));
    }
    Ok(())
}

fn svnr_matches_birth(svnr: &str, birth: NaiveDate) -> Result<bool> {
    let bytes = svnr.as_bytes();
    let day = two(bytes, 4)?;
    let month = two(bytes, 6)?;
    let year = two(bytes, 8)?;
    Ok(day == birth.day()
        && month == birth.month()
        && year == (birth.year().rem_euclid(100) as u32))
}

fn two(bytes: &[u8], at: usize) -> Result<u32> {
    let text = bytes
        .get(at..at + 2)
        .and_then(|s| std::str::from_utf8(s).ok())
        .ok_or_else(|| BizError::Invalid("SVNR date is short".into()))?;
    text.parse::<u32>()
        .map_err(|_| BizError::Invalid("SVNR date is not numeric".into()))
}

fn normalize_code(code: &str) -> Result<String> {
    let c = code.trim().to_ascii_uppercase();
    if c.is_empty() || c.len() > 8 || !c.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(BizError::Invalid(format!(
            "person code {code:?} must be 1-8 letters or digits"
        )));
    }
    Ok(c)
}

fn require_text(value: &str, label: &str, max: usize) -> Result<()> {
    let n = value.trim().chars().count();
    if n == 0 || n > max {
        return Err(BizError::Invalid(format!(
            "{label} must be 1-{max} characters"
        )));
    }
    Ok(())
}

fn check_email(value: &str) -> Result<()> {
    let ok = value.len() <= 120
        && !value.contains(char::is_whitespace)
        && value.split_once('@').is_some_and(|(user, host)| {
            !user.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
        });
    if ok {
        Ok(())
    } else {
        Err(BizError::Invalid(format!("email is not usable: {value:?}")))
    }
}

fn blank(value: Option<&str>) -> bool {
    value.is_none_or(|s| s.trim().is_empty())
}

fn account_name(number: u32) -> Result<String> {
    seed_accounts()
        .into_iter()
        .find(|a| a.number == number)
        .map(|a| a.name.to_string())
        .ok_or_else(|| BizError::Invalid(format!("account {number} is not in the EKR seed")))
}

fn directors(d: &Dossier) -> Vec<&Person> {
    let mut rows: Vec<&Person> = d
        .people
        .iter()
        .filter(|p| p.roles.contains(&Role::ManagingDirector))
        .collect();
    rows.sort_by(|a, b| a.code.cmp(&b.code));
    rows
}

fn impressum(d: &Dossier) -> Result<String> {
    let names = directors(d)
        .iter()
        .map(|p| p.legal_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let fnr = validate_fnr(&d.company.fnr)?;
    let uid = validate_uid(&d.company.uid)?;
    Ok(format!(
        "{name}\n{street}\n{plz} {city}\nRechtsform: {form}\nFirmenbuchgericht: {court}\nFN {fnr}\nUID {uid}\nGeschaeftsfuehrung: {names}\nE-Mail: {email}\nTelefon: {phone}\nWKO: {wko}\n",
        name = d.company.name,
        street = d.company.street,
        plz = d.company.plz,
        city = d.company.city,
        form = d.company.legal_form,
        court = d.company.court,
        email = d.company.email,
        phone = d.company.phone,
        wko = d.company.wko_fachgruppe,
    ))
}

fn handbook(d: &Dossier) -> String {
    let mut text = format!(
        "# {}\n\nSitz: {}, {} {}\n\n## Menschen\n\n",
        d.company.name, d.company.street, d.company.plz, d.company.city
    );
    for person in team_members(d) {
        let position = person.position.as_deref().unwrap_or("");
        let work = person.work_text.as_deref().unwrap_or("");
        text.push_str(&format!(
            "- {} ({}): {}. {}\n",
            person.legal_name, person.code, position, work
        ));
    }
    text.push_str("\n## Fotos\n\n");
    for office in &d.photos.office {
        text.push_str(&format!("- Buero: {office}\n"));
    }
    if let Some(group) = &d.photos.group {
        text.push_str(&format!("- Gruppe: {group}\n"));
    }
    text
}

fn berater_index(d: &Dossier) -> String {
    format!(
        "Beraterpaket {name}\n\nEnthalten:\n- dossier.json\n- buchcraft/opening.json\n- teamcraft/employees.json\n- impressum.txt\n- advisors.json\n\nDie Buchhaltungsfirma reicht UVA, ELDA und den Jahresabschluss ein. Dieses Paket enthaelt keine Portal-Passwoerter.\n",
        name = d.company.name
    )
}

fn json_dossier(d: &Dossier) -> Result<Value> {
    Ok(serde_json::to_value(d)?)
}

fn json_opening(open: &Opening) -> Result<Value> {
    Ok(serde_json::to_value(open)?)
}

fn json_employees(d: &Dossier) -> Result<Value> {
    let mut rows = Vec::new();
    for person in team_members(d) {
        let gaps = card_gaps(person);
        rows.push(json!({
            "code": normalize_code(&person.code)?,
            "legal_name": person.legal_name,
            "source": person.source,
            "candidate_id": person.candidate_id,
            "birth_date": person.birth_date,
            "svnr": person.svnr,
            "address": person.address,
            "email": person.email,
            "phone": person.phone,
            "photo_file": person.photo_file,
            "hire_date": person.hire_date,
            "position": person.position,
            "department": person.department,
            "work_text": person.work_text,
            "weekly_minutes": person.weekly_minutes,
            "leave_days_per_year": 25,
            "kv_name": person.kv_name,
            "kv_group": person.kv_group,
            "employment_kind": person.employment_kind,
            "monthly_gross_cents": person.monthly_gross_cents,
            "iban": person.iban,
            "sonderzahlungen": person.sonderzahlungen,
            "probation_end": person.probation_end,
            "card_complete": gaps.is_empty(),
            "missing": gaps,
        }));
    }
    Ok(json!({ "employees": rows }))
}

fn json_seller(d: &Dossier, fp: &str) -> Result<Value> {
    Ok(json!({
        "name": d.company.name,
        "legal_form": d.company.legal_form,
        "street": d.company.street,
        "plz": d.company.plz,
        "city": d.company.city,
        "court": d.company.court,
        "fnr": validate_fnr(&d.company.fnr)?,
        "uid": validate_uid(&d.company.uid)?,
        "iban": validate_iban(&d.bank.iban)?,
        "email": d.company.email,
        "phone": d.company.phone,
        "logo": d.photos.logo,
        "fingerprint": fp,
    }))
}

fn json_drafts(d: &Dossier) -> Result<Value> {
    let mut drafts = Vec::new();
    for person in team_members(d) {
        drafts.push(json!({
            "id": format!("employment-{}", normalize_code(&person.code)?),
            "kind": "employment",
            "status": "draft",
            "party": person.legal_name,
            "title": person.position,
        }));
    }
    for external in &d.externals {
        drafts.push(json!({
            "id": format!("mandate-{}", external_slug(&external.firm)),
            "kind": "advisor_mandate",
            "status": "draft",
            "party": external.firm,
            "role": external.role,
            "missing_avv": !external.avv_present,
        }));
    }
    Ok(json!({ "drafts": drafts }))
}

fn json_tasks(d: &Dossier) -> Result<Value> {
    let hire = team_members(d)
        .iter()
        .filter_map(|p| p.hire_date)
        .min()
        .unwrap_or(d.company.founding_date);
    let uva = add_months(d.company.founding_date, 2)?;
    let send = d
        .company
        .founding_date
        .checked_add_signed(chrono::TimeDelta::days(4))
        .ok_or(BizError::Overflow)?;
    Ok(json!({
        "project": "Erste 30 Tage",
        "tasks": [
            {"id": "sv-anmeldung", "title": "SV-Anmeldung vor Arbeitsbeginn vorbereiten", "due": hire, "files": false},
            {"id": "uva-first", "title": "Erste UVA fuer die Buchhaltung vorbereiten", "due": uva, "files": false},
            {"id": "send-beraterpaket", "title": "Beraterpaket an die Buchhaltungsfirma senden", "due": send, "files": false}
        ]
    }))
}

fn json_machine(d: &Dossier) -> Result<Value> {
    Ok(json!({
        "tag": d.machine.tag,
        "name": d.machine.name,
        "model": d.machine.model,
        "serial": d.machine.serial,
        "holder_code": normalize_code(&d.machine.holder_code)?,
    }))
}

fn json_advisors(d: &Dossier) -> Result<Value> {
    Ok(serde_json::to_value(&d.externals)?)
}

fn json_desk(d: &Dossier) -> Result<Value> {
    Ok(json!({
        "support_email": d.company.email,
        "smtp": false,
        "drafts_only": true,
    }))
}

fn json_gain(d: &Dossier) -> Result<Value> {
    Ok(json!({
        "vendor": d.company.name,
        "customers": [],
    }))
}

fn json_form(d: &Dossier) -> Result<Value> {
    Ok(json!({
        "id": "contact",
        "company": d.company.name,
        "serve": false,
        "fields": ["name", "email", "message"],
    }))
}

fn json_spend() -> Result<Value> {
    Ok(json!({
        "policy": "Belege vor der Erstattung. Keine historischen Belege aus der Gruendung.",
    }))
}

fn json_helm(d: &Dossier, fp: &str) -> Result<Value> {
    let members = team_members(d);
    let complete = members.iter().filter(|p| card_gaps(p).is_empty()).count();
    Ok(json!({
        "company": d.company.name,
        "fingerprint": fp,
        "paid_in_cents": d.capital.paid_in_cents,
        "headcount": members.len(),
        "cards_complete": complete,
        "open_tasks": 3,
    }))
}

fn add_months(d: NaiveDate, months: i32) -> Result<NaiveDate> {
    let m0 = i32::try_from(d.month0()).map_err(|_| BizError::Overflow)? + months;
    let year = d.year() + m0.div_euclid(12);
    let month = u32::try_from(m0.rem_euclid(12)).map_err(|_| BizError::Overflow)? + 1;
    NaiveDate::from_ymd_opt(year, month, 15).ok_or(BizError::Overflow)
}

fn external_slug(firm: &str) -> String {
    firm.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn file(path: &str, body: &str) -> PackageFile {
    PackageFile {
        relative_path: path.to_string(),
        body: body.to_string(),
    }
}

fn to_pretty(value: &Value) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day)
        .or_else(|| NaiveDate::from_ymd_opt(2026, 1, 1))
        .unwrap_or_default()
}

pub fn package_dir_fingerprint(dir: &Path) -> Result<String> {
    let path = PathBuf::from(dir).join("fingerprint.txt");
    let text = fs::read_to_string(path)?;
    Ok(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svnr_known_value_and_birth() {
        assert_eq!(validate_svnr("1231 120485").unwrap(), "1231120485");
        assert!(validate_svnr("1231120486").is_err());
        assert!(svnr_matches_birth("1231120485", date(1985, 4, 12)).unwrap());
        assert!(!svnr_matches_birth("1231120485", date(1985, 4, 13)).unwrap());
    }

    #[test]
    fn any_austrian_seat_and_any_capital_above_the_floor() {
        let mut graz = sandrasteve_demo();
        graz.vendor = Vendor::Live;
        graz.company.city = "Graz".into();
        graz.company.plz = "8010".into();
        graz.company.court = "Landesgericht fuer ZRS Graz".into();
        graz.capital.mode = CapitalMode::Full;
        graz.capital.subscribed_cents = 5_000_000;
        graz.capital.paid_in_cents = 2_500_000;
        validate(&graz).unwrap();
        assert_eq!(opening(&graz).unwrap().privilege_ends, None);

        let mut flex = graz.clone();
        flex.company.legal_form = "FlexKapG".into();
        flex.capital.mode = CapitalMode::Flex;
        flex.capital.subscribed_cents = 2_000_000;
        flex.capital.paid_in_cents = 500_000;
        validate(&flex).unwrap();

        graz.capital.paid_in_cents = 2_000_000;
        assert!(validate(&graz).is_err());
        flex.capital.mode = CapitalMode::Privileged;
        assert!(validate(&flex).is_err());
    }

    #[test]
    fn privilege_runs_ten_years_and_a_live_company_must_be_read_first() {
        let demo = sandrasteve_demo();
        assert_eq!(
            opening(&demo).unwrap().privilege_ends,
            Some(date(2036, 11, 16))
        );
        let template = startup_template();
        let dir = std::env::temp_dir().join(format!("craft-biz-template-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(write_package(&dir, &template, false)
            .unwrap_err()
            .to_string()
            .contains("prepare"));
        let mut ready = template;
        acknowledge(&mut ready);
        ready.company.uid = "ATU13585627".into();
        let report = write_package(&dir, &ready, false).unwrap();
        assert!(report.changed.iter().any(|p| p == "advice/prepare.md"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sandrasteve_validates_and_balances() {
        let d = sandrasteve_demo();
        validate(&d).unwrap();
        let open = opening(&d).unwrap();
        let debit: i64 = open.lines.iter().map(|l| l.debit_cents).sum();
        let credit: i64 = open.lines.iter().map(|l| l.credit_cents).sum();
        assert_eq!(debit, credit);
        assert_eq!(debit, PRIVILEGED_MIN_PAID_CENTS);
        assert_eq!(open.outstanding_cents, 500_000);
        assert_eq!(open.lines[0].account, 2800);
        assert_eq!(open.lines[1].account, 9000);
    }

    #[test]
    fn externals_stay_out_of_the_employee_file() {
        let d = sandrasteve_demo();
        let files = package(&d).unwrap();
        let employees = files
            .iter()
            .find(|f| f.relative_path == "teamcraft/employees.json")
            .unwrap();
        assert!(employees.body.contains("Alex Berger"));
        assert!(employees.body.contains("Sam Hofer"));
        assert!(!employees.body.contains("Kanzlei Muster"));
        assert!(!employees.body.contains("Beratung Nord"));
        let drafts = files
            .iter()
            .find(|f| f.relative_path == "contractcraft/drafts.json")
            .unwrap();
        assert!(drafts.body.contains("\"missing_avv\": true"));
        let impressum = files
            .iter()
            .find(|f| f.relative_path == "impressum.txt")
            .unwrap();
        assert!(impressum.body.contains("FN 123456a"));
        assert!(impressum.body.contains("ATU13585627"));
        assert!(impressum.body.contains("Alex Berger"));
    }

    #[test]
    fn fingerprint_changes_when_the_uid_changes() {
        let d = sandrasteve_demo();
        let first = fingerprint(&d).unwrap();
        let mut other = d;
        other.company.uid = "ATU12345675".into();
        let second = fingerprint(&other).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn george_export_posts_no_opening_lines() {
        let mut d = sandrasteve_demo();
        d.bank.george_csv = Some("george.csv".into());
        let open = opening(&d).unwrap();
        assert!(open.lines.is_empty());
    }

    #[test]
    fn founding_card_without_photo_is_rejected() {
        let mut d = sandrasteve_demo();
        d.people[1].photo_file = None;
        assert!(validate(&d).unwrap_err().to_string().contains("photo"));
    }

    #[test]
    fn shareholder_only_is_not_an_employee() {
        let mut d = sandrasteve_demo();
        d.people.push(Person {
            code: "HOLD".into(),
            roles: vec![Role::Shareholder],
            source: PersonSource::Onboarding,
            asvg_wage: false,
            legal_name: "Ruth Holding".into(),
            birth_date: date(1970, 1, 1),
            svnr: None,
            address: "Wien".into(),
            email: "ruth@example.com".into(),
            phone: "+43 1 9".into(),
            photo_file: None,
            hire_date: None,
            position: None,
            department: None,
            work_text: None,
            weekly_minutes: None,
            kv_name: None,
            kv_group: None,
            employment_kind: None,
            monthly_gross_cents: None,
            iban: None,
            sonderzahlungen: false,
            shares: Some(10),
            drawing_rights: None,
            candidate_id: None,
            probation_end: None,
        });
        d.people[0].shares = Some(90);
        validate(&d).unwrap();
        let body = package(&d)
            .unwrap()
            .into_iter()
            .find(|f| f.relative_path == "teamcraft/employees.json")
            .unwrap()
            .body;
        assert!(!body.contains("Ruth Holding"));
        assert!(fingerprint(&d).unwrap().contains("employees=2"));
    }

    #[test]
    fn second_write_changes_nothing() {
        let dir = std::env::temp_dir().join(format!("craft-biz-company-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let d = sandrasteve_demo();
        let first = write_package(&dir, &d, true).unwrap();
        assert!(!first.changed.is_empty());
        assert!(first.unchanged.is_empty());
        let second = write_package(&dir, &d, true).unwrap();
        assert!(second.changed.is_empty());
        assert_eq!(second.unchanged.len(), first.changed.len());
        assert_eq!(first.fingerprint, second.fingerprint);
        assert!(write_package(&dir, &d, false).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn later_hire_may_ship_an_incomplete_card() {
        let mut d = sandrasteve_demo();
        d.people.push(Person {
            code: "NEW1".into(),
            roles: vec![Role::Employee],
            source: PersonSource::Hirecraft,
            asvg_wage: true,
            legal_name: "Neu Eingestellt".into(),
            birth_date: date(1999, 2, 2),
            svnr: Some(svnr_for(1999, 2, 2, [1, 0, 0])),
            address: String::new(),
            email: "neu@sandrasteve.example".into(),
            phone: "+43 1 3".into(),
            photo_file: None,
            hire_date: Some(date(2026, 12, 1)),
            position: Some("Entwickler".into()),
            department: Some("Produkt".into()),
            work_text: Some("Kommt aus HireCraft.".into()),
            weekly_minutes: Some(2310),
            kv_name: Some("IT-KV".into()),
            kv_group: Some("ST1".into()),
            employment_kind: Some(EmploymentKind::Angestellt),
            monthly_gross_cents: Some(300_000),
            iban: None,
            sonderzahlungen: true,
            shares: None,
            drawing_rights: None,
            candidate_id: Some("NEW1".into()),
            probation_end: None,
        });
        validate(&d).unwrap();
        let body = package(&d)
            .unwrap()
            .into_iter()
            .find(|f| f.relative_path == "teamcraft/employees.json")
            .unwrap()
            .body;
        assert!(body.contains("\"card_complete\": false"));
        assert!(body.contains("Neu Eingestellt"));
    }

    fn svnr_for(year: i32, month: u32, day: u32, serial: [u32; 3]) -> String {
        let yy = (year.rem_euclid(100)) as u32;
        let mut digits = vec![
            serial[0],
            serial[1],
            serial[2],
            0,
            day / 10,
            day % 10,
            month / 10,
            month % 10,
            yy / 10,
            yy % 10,
        ];
        let weights = [3u32, 7, 9, 0, 5, 8, 4, 2, 1, 6];
        let sum: u32 = digits.iter().zip(weights).map(|(d, w)| d * w).sum();
        let check = sum % 11;
        assert!(check < 10, "pick another serial");
        digits[3] = check;
        digits
            .into_iter()
            .map(|d| char::from(b'0' + d as u8))
            .collect()
    }
}
