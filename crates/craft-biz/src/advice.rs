//! Orientation a founder reads before typing a company.
//!
//! This is not legal advice, tax advice, or a filing. The Notar and the
//! Steuerberatung decide. The text is the same for every small company.
//! sandrasteve GmbH is only a labelled demo.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

const SECTIONS: &[Section] = &[
    Section {
        id: "before-you-type",
        title: "Lesen, dann eintragen",
        body: "SetupCraft bereitet eine kleine österreichische Kapitalgesellschaft vor. Danach erfassen Sie Ihre eigene Firma. Jede Firma trägt ihre eigenen Zahlen, Personen und Berater ein.\n\nDie Demo sandrasteve GmbH ist ein beschriftetes Beispiel mit Prüfziffern. Sie ist keine eingetragene Gesellschaft.\n\nDas hier ist Orientierung. Es ist keine Rechtsberatung und keine Steuerberatung. Notar und Steuerberatung entscheiden. Dieses Programm meldet nichts an das Firmenbuch, an FinanzOnline oder an ELDA.",
    },
    Section {
        id: "rechtsform",
        title: "Welche Hülle",
        body: "Einzelunternehmen: Sie haften mit dem Privatvermögen. Dafür gibt es kein Stammkapital. Dieses Dossier bildet eine Kapitalgesellschaft ab.\n\nGmbH: Stammkapital mindestens 35.000 Euro. Eingezahlt werden mindestens die Hälfte und mindestens 17.500 Euro. Mehr Kapital ist zulässig. Die Einzahlungsquote setzt der Notar in den Gesellschaftsvertrag.\n\nGründungsprivilegierung nach GmbHG Paragraph 10b: Sie bringen vorerst mindestens 10.000 Euro auf, davon mindestens 5.000 Euro in bar. Das Privileg dauert zehn Jahre ab der Eintragung. Danach ist auf das gesetzliche Stammkapital aufzustocken. Die Gewinnausschüttung ist während des Privilegs beschränkt.\n\nFlexKapG, auch FlexCo, seit 1. Jänner 2024: Mindeststammkapital 10.000 Euro und Mindesteinzahlung 5.000 Euro. Das ist eine dauerhafte Regel, kein Zehn-Jahres-Privileg. Sie passt, wenn später Unternehmenswert-Anteile für Mitarbeiter im Vertrag stehen sollen. Der Gründungsakt bleibt ein Notariatsakt.\n\nDeutschland kennt die UG (haftungsbeschränkt). Österreich kennt sie nicht. Eine österreichische Gründung hier ist GmbH, gründungsprivilegierte GmbH oder FlexKapG.",
    },
    Section {
        id: "sitz",
        title: "Firma, Sitz, Gericht",
        body: "Die Firma muss sich von bestehenden Firmen unterscheiden und den Rechtsformzusatz tragen: GmbH oder FlexKapG. Der Sitz ist der Ort der Hauptverwaltung.\n\nDas Firmenbuchgericht folgt dem Sitz. In Wien ist es das Handelsgericht Wien, sonst das Landesgericht am Sitz. Eine Wiener PLZ liegt zwischen 1010 und 1230. Jeder andere österreichische Sitz trägt seine eigene PLZ und sein Gericht.\n\nPrüfen Sie den Namen, bevor der Notar schreibt. Das Firmenbuch weist eine verwechselbare Firma zurück.",
    },
    Section {
        id: "kapital",
        title: "Kapital und Bank",
        body: "Eröffnen Sie ein Konto auf die Gesellschaft in Gründung. Die Bankbestätigung über die einbezahlte Stammeinlage ist eine Beilage zur Firmenbuchanmeldung. Die Einzahlung erfolgt, bevor der Notar anmeldet.\n\nLiegt die Einzahlung schon in einem George-Export, tragen Sie den Dateipfad ein. Dann entsteht keine zweite Eröffnungsbuchung.\n\nWas noch nicht einbezahlt ist, wird ausgewiesen und nicht gebucht. Der mitgelieferte Kontenrahmen hat kein Konto für ausstehende Einlagen. Die Steuerberatung setzt dieses Konto.",
    },
    Section {
        id: "personen",
        title: "Gesellschafter, Geschäftsführung, Personal, Externe",
        body: "Gesellschafter halten die Anteile. Die Geschäftsführung vertritt die Gesellschaft. Eine Person kann beides sein. Die Vertretung ist selbständig oder kollektiv, so wie der Gesellschaftsvertrag sie fasst.\n\nEin Gesellschafter-Geschäftsführer ist versicherungsrechtlich oft bei der SVS nach GSVG, manchmal als Dienstnehmer bei der ÖGK nach ASVG. Das hängt an Beteiligung, Weisung und dem Dienstvertrag. Im Dossier steht nur, ob ein ASVG-Lohn gezahlt wird. Die Zuordnung klärt die Steuerberatung vor der Anmeldung.\n\nArbeitnehmer erhalten eine Mitarbeiterkarte: Foto, SVNR, Adresse, Eintritt, Funktion, die aktuelle Arbeit, Stunden, Kollektivvertrag, Brutto. Die Anmeldung eines Dienstnehmers hat vor dem ersten Arbeitstag zu erfolgen. Sie legen die Daten hier ab. Die Buchhaltung meldet.\n\nBuchhaltung, Unternehmensberatung, Notar, Rechtsanwalt, Bank und Versicherung sind Externe. Sie werden keine Arbeitnehmer und erscheinen nicht in der Zeitaufzeichnung. Fehlt ein Auftragsverarbeitungsvertrag, bleibt das am Mandatsentwurf sichtbar.",
    },
    Section {
        id: "ablauf",
        title: "Notar und Firmenbuch",
        body: "Eine brauchbare Reihenfolge: Gesellschaftsvertrag entwerfen, Konto und Kapitalnachweis, Notariatsakt, Firmenbuchanmeldung, Beschluss des Gerichts. Die Gesellschaft besteht mit der Eintragung.\n\nKosten fallen als Notar, Gerichtsgebühr und Veröffentlichung an. Die Beträge stehen in den jeweils gültigen Tarifen. Den aktuellen Satz nennt der Notar.\n\nDer Vertrag nennt Firma, Sitz, Gegenstand, Stammkapital, Gesellschafter, Geschäftsführung und die Vertretung.",
    },
    Section {
        id: "nach-der-eintragung",
        title: "Steuer, Kammer, erste Belege",
        body: "Nach der Eintragung folgen UID und Steuernummer. Ob Kleinunternehmer oder Regelbesteuerung gilt, steht im Umsatzsteuergesetz und im Bescheid. Die Grenze ändert sich. Die Steuerberatung setzt die Variante. Das Programm legt die Aufgabe, die erste UVA vorzubereiten. Es sendet sie nicht.\n\nMit dem Gewerbe entsteht die Mitgliedschaft in der Wirtschaftskammer. Die Fachgruppe folgt dem Gewerbewortlaut. ÖNACE und Fachgruppe tragen Sie so ein, wie die Kammer sie führt.\n\nLohnverrechnung ist ein späteres Programm. Bis dahin sammelt das Beraterpaket die Unterlagen, und die Kanzlei rechnet ab und meldet.",
    },
    Section {
        id: "was-sie-eintragen",
        title: "Ihre Daten, nicht die Demo",
        body: "Nach dem Lesen ersetzen Sie jede Vorgabe: Firma, Rechtsform, Sitz, PLZ, Gericht, FN, UID, ÖNACE, Finanzamt, Wirtschaftsjahr, Gründungsdatum, Fachgruppe, E-Mail, Telefon, Kapital, Bank, Personen, Externe, Fotodateien, dieser PC.\n\nFotos bleiben Dateinamen, die Sie zuordnen. Es gibt keine Gesichtserkennung.\n\nDie Kenntnisnahme gilt, wenn diese Abschnitte gelesen sind. Erst danach schreibt ein echtes Dossier das Paket. Die Demo schreibt nur mit ausdrücklicher Freigabe.",
    },
];

pub fn sections() -> &'static [Section] {
    SECTIONS
}

pub fn markdown() -> String {
    let mut text = String::from(
        "# Vorbereitung auf die Gründung\n\nLesen Sie die Abschnitte. Danach tragen Sie Ihre Firma ein.\n\n",
    );
    for (index, section) in SECTIONS.iter().enumerate() {
        text.push_str(&format!(
            "## {}. {}\n\n{}\n\n",
            index + 1,
            section.title,
            section.body
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guide_names_the_austrian_choices_and_refuses_the_german_ug() {
        let text = markdown();
        assert!(text.contains("FlexKapG"));
        assert!(text.contains("Gründungsprivilegierung"));
        assert!(text.contains("35.000 Euro"));
        assert!(text.contains("10.000 Euro"));
        assert!(text.contains("Österreich kennt sie nicht"));
        assert!(text.contains("Handelsgericht Wien"));
        assert!(text.contains("ELDA"));
        assert!(sections().len() >= 8);
        let mut ids: Vec<&str> = sections().iter().map(|s| s.id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }
}
