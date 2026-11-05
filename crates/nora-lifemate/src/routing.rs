//! Experimental candidate-tool routing, never an action or permission decision.
//! Scores are lexical evidence, not calibrated probabilities or a recall claim.
//! Unknown/deictic requests keep the full supplied catalog. SDK authority stays
//! with the caller; this module can only return tools it was given.

use crate::protocol::{HistoryTurn, ToolSpec};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingConfidence {
    LexicalEvidence,
    HistoryReference,
    FullCatalogFallback,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutingReport {
    /// Keep schema duplication out of a trace's routing summary.
    #[serde(skip)]
    pub tools: Vec<ToolSpec>,
    pub selected_apps: Vec<String>,
    pub matched_apps: Vec<String>,
    pub confidence: RoutingConfidence,
    pub fallback: bool,
    pub reason: &'static str,
    pub evidence: Vec<String>,
    pub app_scores: BTreeMap<String, u32>,
}

fn normalize(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars().flat_map(char::to_lowercase) {
        let plain = match character {
            'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ'
            | 'ặ' | 'ẳ' | 'ẵ' => 'a',
            'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => {
                'e'
            }
            'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
            'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ'
            | 'ợ' | 'ở' | 'ỡ' => 'o',
            'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => {
                'u'
            }
            'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
            'đ' => 'd',
            '\u{0300}'..='\u{036f}' => continue,
            value if value.is_alphanumeric() => value,
            _ => ' ',
        };
        output.push(plain);
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains(padded: &str, phrase: &str) -> bool {
    !phrase.is_empty() && padded.contains(&format!(" {phrase} "))
}

fn phrases(app: &str) -> &'static [&'static str] {
    match app {
        "smart_home" => &[
            "den",
            "quat",
            "dieu hoa",
            "may lanh",
            "nhiet do",
            "do sang",
            "o cam",
            "rem cua",
            "nha thong minh",
            "ngu canh",
        ],
        "home_security" => &[
            "camera",
            "an ninh",
            "bao ve",
            "canh bao",
            "bao dong",
            "chong trom",
        ],
        "door_lock" => &[
            "khoa cua",
            "mo cua",
            "mo khoa",
            "o khoa",
            "khoa lai",
            "ma mo cua",
            "khoa cong",
        ],
        "robot_vacuum" => &[
            "robot",
            "hut bui",
            "don nha",
            "lau nha",
            "ve dock",
            "ve tram sac",
        ],
        "appliances" => &[
            "may giat",
            "may say",
            "may rua bat",
            "may rua chen",
            "lo vi song",
            "lo nuong",
            "chu trinh",
            "giat",
            "say quan ao",
            "may pha ca phe",
        ],
        "home_energy" => &[
            "dien tieu thu",
            "tieu thu dien",
            "kwh",
            "nang luong",
            "cong suat",
            "nguong dien",
        ],
        "alarm" => &["bao thuc", "chuong bao", "danh thuc", "alarm"],
        "reminders" => &["nhac", "nhac viec", "nhac nho", "reminder"],
        "calendar" => &[
            "lich", "su kien", "cuoc hop", "cuoc hen", "cuoc gap", "hen gap",
        ],
        "notes" => &["ghi chu", "ghi lai", "luu ghi chu", "note"],
        "tasks" => &[
            "viec can lam",
            "danh sach viec",
            "nhiem vu",
            "to do",
            "todo",
            "viec da xong",
        ],
        "contacts" => &["danh ba", "lien he", "so dien thoai", "doi so", "doi email"],
        "phone" => &["goi", "cuoc goi", "goi nho", "goi thoai", "dien thoai cho"],
        "sms" => &["sms", "tin nhan", "nhan", "gui tin", "gui loi nhan"],
        "chat_ott" => &[
            "zalo",
            "ott",
            "messenger",
            "whatsapp",
            "cuoc tro chuyen",
            "nhom chat",
            "nhom zalo",
            "goi video",
            "gui vi tri",
        ],
        "email" => &[
            "email",
            "mail",
            "thu dien tu",
            "hop thu",
            "thu nhap",
            "thu chua doc",
            "thu da gui",
        ],
        "video_meeting" => &[
            "hop truc tuyen",
            "phong hop",
            "tham gia hop",
            "vao hop",
            "video meeting",
            "zoom",
            "google meet",
        ],
        "banking" => &[
            "ngan hang",
            "tai khoan",
            "chuyen khoan",
            "chuyen tien",
            "tiet kiem",
            "thu huong",
            "so du",
            "giao dich",
        ],
        "ewallet" => &[
            "vi",
            "nap tien",
            "nap dien thoai",
            "nap cho",
            "nap giup",
            "nap dum",
            "chuyen tien",
            "thanh toan qr",
            "ma qr",
            "so du",
        ],
        "bill_payment" => &[
            "hoa don",
            "tien dien",
            "tien nuoc",
            "tien mang",
            "cuoc dien thoai",
            "thanh toan tu dong",
            "dien thang",
            "nuoc thang",
            "mang thang",
        ],
        "loyalty" => &[
            "voucher",
            "diem thuong",
            "diem thanh vien",
            "doi diem",
            "uu dai",
            "phieu giam gia",
        ],
        "weather" => &[
            "thoi tiet",
            "du bao",
            "mua",
            "troi",
            "nhiet do ngoai troi",
            "nang",
            "bao nhieu do",
        ],
        "gold_price" => &["gia vang", "vang sjc", "vang nhan", "vang mieng", "gold"],
        "fuel_price" => &["gia xang", "xang", "gia dau", "nhien lieu", "ron", "e5"],
        "lottery" => &[
            "xo so",
            "ve so",
            "do so",
            "trung thuong",
            "ket qua theo dai",
        ],
        "exchange_rate" => &[
            "ty gia",
            "quy doi tien",
            "doi tien",
            "ngoai te",
            "usd",
            "eur",
            "jpy",
            "do la",
        ],
        "news" => &[
            "tin tuc",
            "tin nong",
            "tin noi bat",
            "ban tin",
            "thoi su",
            "doc bao",
            "bai bao",
            "tim tin",
            "tin ve",
            "bai viet",
        ],
        "calculator" => &[
            "tinh",
            "cong",
            "tru",
            "nhan",
            "chia",
            "bieu thuc",
            "phep tinh",
            "doi don vi",
            "quy doi don vi",
        ],
        "maps" => &[
            "chi duong",
            "duong di",
            "tuyen duong",
            "dia diem",
            "dia chi",
            "tim cho",
            "quanh day",
            "ban do",
            "di duong nao",
        ],
        "food_delivery" => &[
            "dat do an",
            "dat mon",
            "dat com",
            "giao do an",
            "thuc don",
            "menu",
            "nha hang",
            "quan an",
            "mon an",
            "don do an",
            "huy don",
        ],
        "ride_hailing" => &[
            "dat xe",
            "goi xe",
            "don xe",
            "xe bon cho",
            "xe 4 cho",
            "xe bay cho",
            "xe 7 cho",
            "chuyen xe",
            "taxi",
            "grab",
            "bao gia xe",
        ],
        "flights" => &[
            "chuyen bay",
            "may bay",
            "ve bay",
            "ve may bay",
            "san bay",
            "check in",
            "checkin",
            "lam thu tuc bay",
        ],
        "shopping_list" => &[
            "danh sach mua",
            "can mua",
            "da mua",
            "mua sam",
            "do can mua",
            "di cho",
            "danh dau da mua",
        ],
        "package_tracking" => &[
            "buu kien",
            "kien hang",
            "van don",
            "tracking",
            "buu ta",
            "giao hang",
            "lay hang",
            "gui hang",
            "don hang",
        ],
        "music" => &[
            "am nhac",
            "bai hat",
            "mo nhac",
            "nghe nhac",
            "phat nhac",
            "playlist",
            "danh sach nhac",
            "chuyen bai",
            "bai tiep theo",
            "ca khuc",
        ],
        "tv" => &[
            "tivi",
            "tv",
            "truyen hinh",
            "kenh",
            "vtv",
            "htv",
            "ghi hinh",
            "phat song",
        ],
        "podcast_radio" => &[
            "podcast",
            "radio",
            "dai phat thanh",
            "kenh radio",
            "hang doi nghe",
            "theo doi kenh",
        ],
        "medication" => &["thuoc", "uong thuoc", "nhac thuoc", "lieu thuoc", "da uong"],
        "fitness" => &[
            "buoc chan",
            "so buoc",
            "calo",
            "calorie",
            "buoi tap",
            "tap luyen",
            "luyen tap",
            "chay bo",
            "dap xe",
            "the duc",
            "muc tieu buoc",
        ],
        "learning" => &[
            "khoa hoc",
            "bai hoc",
            "buoi hoc",
            "dang ky hoc",
            "tien do hoc",
            "hoc tap",
        ],
        "bike_rental" => &[
            "thue xe dap",
            "giu xe dap",
            "tram xe",
            "xe dap cong cong",
            "tim xe dap",
        ],
        "home_automation" => &[
            "tu dong",
            "quy tac",
            "moi ngay bat",
            "moi ngay tat",
            "hang ngay bat",
            "hang ngay tat",
        ],
        "household_chores" => &[
            "viec nha",
            "giao viec",
            "phan cong",
            "nguoi phu trach",
            "thanh vien gia dinh",
        ],
        "library" => &[
            "cuon sach",
            "quyen sach",
            "doc sach",
            "muon sach",
            "tac gia",
            "thu vien",
            "tim sach",
            "dat sach",
        ],
        "subscriptions" => &[
            "goi dang ky",
            "huy dang ky",
            "gia han",
            "goi thue bao",
            "subscription",
            "dich vu dang ky",
            "ngung dang ky",
            "goi dich vu",
        ],
        _ => &[],
    }
}

fn app_name(tool: &ToolSpec) -> &str {
    tool.name
        .split_once('.')
        .map_or(tool.name.as_str(), |(app, _)| app)
}

fn meaningful(word: &str) -> bool {
    word.len() >= 4
        && !matches!(
            word,
            "danh"
                | "sach"
                | "thong"
                | "trang"
                | "thai"
                | "nhat"
                | "nhung"
                | "nhieu"
                | "trong"
                | "ngoai"
                | "khong"
                | "truyen"
                | "theo"
                | "nguoi"
                | "duoc"
                | "buoc"
                | "tham"
                | "nhan"
                | "hien"
                | "dung"
                | "hoac"
                | "truong"
                | "truoc"
                | "ngay"
                | "dang"
                | "xong"
                | "thanh"
                | "toan"
                | "khop"
                | "bang"
                | "loai"
                | "cung"
                | "tung"
                | "nay"
                | "khac"
                | "dien"
                | "tuong"
        )
}

fn xrefs(value: &Value, output: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("x-ref").and_then(Value::as_str)
                && let Some((app, _)) = reference.split_once('.')
            {
                output.insert(app.to_owned());
            }
            for child in object.values() {
                xrefs(child, output);
            }
        }
        Value::Array(array) => array.iter().for_each(|child| xrefs(child, output)),
        _ => {}
    }
}

fn prior_apps(history: &[HistoryTurn], available: &BTreeSet<String>) -> BTreeSet<String> {
    for turn in history.iter().rev().filter(|turn| turn.role == "assistant") {
        let Some(calls) = turn.tool_calls.as_ref().and_then(Value::as_array) else {
            continue;
        };
        let apps: BTreeSet<_> = calls
            .iter()
            .filter_map(|call| call.get("name").and_then(Value::as_str))
            .filter_map(|name| name.split_once('.').map(|(app, _)| app.to_owned()))
            .filter(|app| available.contains(app) && app != "sys")
            .collect();
        if !apps.is_empty() {
            return apps;
        }
    }
    BTreeSet::new()
}

/// Select candidate apps and their data dependencies. Negation, conditions and
/// authorization are intentionally not decided here; no action is executed.
pub fn route_tools(latest: &str, history: &[HistoryTurn], tools: &[ToolSpec]) -> RoutingReport {
    let available: BTreeSet<String> = tools
        .iter()
        .map(app_name)
        .filter(|app| *app != "sys")
        .map(str::to_owned)
        .collect();
    let text = normalize(latest);
    let padded = format!(" {text} ");
    let mut scores = BTreeMap::new();
    let mut evidence = BTreeSet::new();
    let mut matched = BTreeSet::new();
    let fallback = |reason,
                    scores: BTreeMap<String, u32>,
                    evidence: BTreeSet<String>,
                    matched: BTreeSet<String>| RoutingReport {
        tools: tools.to_vec(),
        selected_apps: available.iter().cloned().collect(),
        matched_apps: matched.into_iter().collect(),
        confidence: RoutingConfidence::FullCatalogFallback,
        fallback: true,
        reason,
        evidence: evidence.into_iter().collect(),
        app_scores: scores,
    };
    if text.is_empty() {
        return fallback("empty_request", scores, evidence, matched);
    }
    if ["tat ca ung dung", "moi ung dung", "toan bo he thong"]
        .iter()
        .any(|phrase| contains(&padded, phrase))
    {
        return fallback("broad_system_request", scores, evidence, matched);
    }

    // Rare description terms supplement, rather than replace, the domain
    // lexicon. At least two distinct rare terms are needed to narrow a catalog.
    let mut descriptions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for tool in tools {
        let app = app_name(tool);
        if app == "sys" {
            continue;
        }
        descriptions.entry(app.to_owned()).or_default().extend(
            normalize(&tool.description)
                .split_whitespace()
                .filter(|word| meaningful(word))
                .map(str::to_owned),
        );
    }
    let mut frequencies: BTreeMap<String, usize> = BTreeMap::new();
    for words in descriptions.values() {
        for word in words {
            *frequencies.entry(word.clone()).or_default() += 1;
        }
    }
    let query: BTreeSet<&str> = text.split_whitespace().collect();
    for app in &available {
        let mut score = 0;
        if contains(&padded, &normalize(app)) {
            score += 4;
            evidence.insert(format!("app_name:{app}"));
        }
        for phrase in phrases(app) {
            if contains(&padded, phrase) {
                score += 4;
                evidence.insert(format!("lexicon:{app}:{phrase}"));
            }
        }
        let rare: Vec<_> = descriptions
            .get(app)
            .into_iter()
            .flatten()
            .filter(|word| query.contains(word.as_str()) && frequencies.get(*word) == Some(&1))
            .collect();
        if rare.len() >= 2 {
            score += 4;
            evidence.insert(format!(
                "catalog_description:{app}:{}",
                rare.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if score >= 4 {
            matched.insert(app.clone());
        }
        if score > 0 {
            scores.insert(app.clone(), score);
        }
    }
    let mut confidence = RoutingConfidence::LexicalEvidence;
    if matched.is_empty() {
        let deictic = [
            "cai do",
            "cai nay",
            "no",
            "nguoi do",
            "cho do",
            "som nhat",
            "muon nhat",
            "cai truoc",
            "nhu truoc",
            "doi lai",
            "dat lai",
            "xoa di",
        ]
        .iter()
        .any(|phrase| contains(&padded, phrase));
        if deictic {
            let previous = prior_apps(history, &available);
            if previous.len() == 1 {
                for app in previous {
                    scores.insert(app.clone(), 4);
                    evidence.insert(format!("history_tool_app:{app}"));
                    matched.insert(app);
                }
                confidence = RoutingConfidence::HistoryReference;
            } else {
                return fallback(
                    "unresolved_or_ambiguous_history_reference",
                    scores,
                    evidence,
                    matched,
                );
            }
        } else {
            return fallback("insufficient_domain_evidence", scores, evidence, matched);
        }
    }
    let mut selected = matched.clone();
    let conditional = ["neu", "chi khi", "tru khi", "con khong"]
        .iter()
        .any(|phrase| contains(&padded, phrase));
    if conditional && selected.contains("calendar") {
        for app in ["weather", "contacts", "sms"] {
            if available.contains(app) {
                selected.insert(app.into());
                evidence.insert(format!("conditional_schedule_dependency:{app}"));
            }
        }
    }
    loop {
        let before = selected.len();
        let mut dependencies = BTreeSet::new();
        for tool in tools
            .iter()
            .filter(|tool| selected.contains(app_name(tool)))
        {
            xrefs(&tool.parameters, &mut dependencies);
        }
        // Some catalogs describe a dependency without an x-ref annotation.
        for (app, needed) in [
            ("phone", "contacts"),
            ("sms", "contacts"),
            ("chat_ott", "contacts"),
            ("ewallet", "contacts"),
            ("ewallet", "food_delivery"),
        ] {
            if selected.contains(app) {
                dependencies.insert(needed.into());
            }
        }
        for dependency in dependencies {
            if available.contains(&dependency) && selected.insert(dependency.clone()) {
                evidence.insert(format!("data_dependency:{dependency}"));
            }
        }
        if before == selected.len() {
            break;
        }
    }
    let routed: Vec<_> = tools
        .iter()
        .filter(|tool| selected.contains(app_name(tool)) || tool.name == "sys.ask_missing_info")
        .cloned()
        .collect();
    if routed.is_empty() {
        return fallback("no_supplied_tools_for_route", scores, evidence, matched);
    }
    RoutingReport {
        tools: routed,
        selected_apps: selected.into_iter().collect(),
        matched_apps: matched.into_iter().collect(),
        confidence,
        fallback: false,
        reason: "candidate_apps_with_dependencies",
        evidence: evidence.into_iter().collect(),
        app_scores: scores,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: name.into(),
            kind: "read".into(),
            parameters: json!({"type":"object","properties":{}}),
            returns: Value::Null,
        }
    }
    fn catalog() -> Vec<ToolSpec> {
        [
            "calendar.list_events",
            "weather.get_forecast",
            "contacts.search",
            "sms.send",
            "phone.call",
            "music.play",
            "sys.ask_missing_info",
        ]
        .into_iter()
        .map(tool)
        .collect()
    }
    #[test]
    fn conditional_schedule_keeps_weather_and_communication_dependencies() {
        let report = route_tools(
            "Nếu trời mưa thì dời lịch rồi nhắn SMS báo giờ mới.",
            &[],
            &catalog(),
        );
        assert!(!report.fallback);
        for app in ["calendar", "weather", "contacts", "sms"] {
            assert!(report.selected_apps.iter().any(|value| value == app));
        }
        assert!(!report.selected_apps.iter().any(|app| app == "music"));
        assert!(
            report
                .tools
                .iter()
                .any(|tool| tool.name == "sys.ask_missing_info")
        );
    }
    #[test]
    fn phone_route_keeps_contacts_but_never_adds_unsupplied_tools() {
        let supplied = catalog();
        let route = route_tools("Gọi cho đồng nghiệp giúp tôi", &[], &supplied);
        assert!(route.selected_apps.iter().any(|app| app == "contacts"));
        assert!(route.selected_apps.iter().any(|app| app == "phone"));
        assert!(
            route
                .tools
                .iter()
                .all(|tool| supplied.iter().any(|original| original.name == tool.name))
        );
        let only_phone = vec![tool("phone.call")];
        assert_eq!(route_tools("Gọi điện", &[], &only_phone).tools.len(), 1);
    }
    #[test]
    fn uncertain_or_unresolved_reference_preserves_the_full_catalog() {
        let tools = catalog();
        for utterance in [
            "Làm giúp tôi chuyện ấy",
            "Xóa cái sớm nhất",
            "",
            "một tác vụ không có từ khóa",
        ] {
            let route = route_tools(utterance, &[], &tools);
            assert!(route.fallback, "{utterance}");
            assert_eq!(
                route
                    .tools
                    .iter()
                    .map(|tool| &tool.name)
                    .collect::<Vec<_>>(),
                tools.iter().map(|tool| &tool.name).collect::<Vec<_>>()
            );
        }
    }
    #[test]
    fn retraction_does_not_remove_the_remaining_weather_query_or_execute_anything() {
        let tools = catalog();
        let before = serde_json::to_value(&tools).unwrap();
        let route = route_tools(
            "Đừng hủy lịch nữa, chỉ xem giúp thời tiết ngày mai.",
            &[],
            &tools,
        );
        assert!(
            route
                .tools
                .iter()
                .any(|tool| tool.name == "weather.get_forecast")
        );
        assert_eq!(serde_json::to_value(&tools).unwrap(), before);
        assert!(serde_json::to_value(&route).unwrap().get("tools").is_none());
    }
    #[test]
    fn explicit_single_app_history_can_resolve_a_deictic_followup() {
        let history = vec![HistoryTurn {
            role: "assistant".into(),
            text: None,
            audio: None,
            tool_calls: Some(json!([{"name":"calendar.list_events","arguments":{}}])),
            tool_results: None,
        }];
        let route = route_tools("Xóa cái sớm nhất", &history, &catalog());
        assert!(!route.fallback);
        assert_eq!(route.confidence, RoutingConfidence::HistoryReference);
        assert_eq!(route.matched_apps, ["calendar"]);
        let mut ambiguous = history;
        ambiguous[0].tool_calls =
            Some(json!([{"name":"calendar.list_events"},{"name":"phone.call"}]));
        assert!(route_tools("Xóa cái sớm nhất", &ambiguous, &catalog()).fallback);
    }
    #[test]
    fn vietnamese_precomposed_and_combining_accents_route_identically() {
        assert_eq!(normalize("ĐIỆN THOẠI"), "dien thoai");
        assert_eq!(
            normalize("đie\u{0323}\u{0302}n thoa\u{0323}i"),
            "dien thoai"
        );
    }
}
