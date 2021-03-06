//! Misc funcion to parse query.

use regex::Regex;

use crate::types::SubReportStatus;

#[doc(hidden)]
#[derive(Default)]
pub struct ParsedQuery<'a> {
    pub name: Option<&'a str>,
    pub residence_address: Option<&'a str>,
    pub age: Option<i32>,
    pub gender: Option<&'a str>,
    pub come_from: Option<&'a str>,
    pub status: Option<SubReportStatus>,
    pub village_name: Option<&'a str>,
    pub district_name: Option<&'a str>,
    pub query: String,
}

/// Parse query to get "semantic"-like search
pub fn parse_query<'a>(query: &'a str) -> ParsedQuery<'a> {
    let s: Vec<&str> = query.split(' ').collect();

    let name = s
        .iter()
        .find(|a| !a.contains(':') || a.starts_with("nama:"))
        .cloned();

    let residence_address = value_str_opt!(s, "tt");
    let age = value_str_opt!(s, "umur").and_then(|a| a.parse::<i32>().ok());
    let gender = value_str_opt!(s, "jk");
    let come_from = value_str_opt!(s, "dari");
    let status: Option<SubReportStatus> = value_str_opt!(s, "status").map(|a| SubReportStatus::from(a));
    let village_name = value_str_opt!(s, "desa");
    let district_name = value_str_opt!(s, "kcm");

    // let re = Regex::new("(nama|tt|umur|jk|dari|status|desa|kcm)\\:\\w+").expect("cannot compile regex");
    let final_query = query
        .split(" ")
        .filter(|a| !a.contains(':'))
        .collect::<Vec<&str>>()
        .join(" ");

    ParsedQuery {
        name,
        residence_address,
        age,
        gender,
        come_from,
        status,
        village_name,
        district_name,
        query: final_query,
    }
}
