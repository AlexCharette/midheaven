//! Russian vocabulary. Match terms are lowercase and list the common case
//! forms a spoken reading uses (nominative + genitive/prepositional), because
//! the router matches whole words, not stems. Native-speaker review welcome —
//! extending a term list never changes ids or geometry, only what routes.

use super::{AspectEntry, Entry, HouseEntry, LocaleTable, PdfChrome};

pub static TABLE: LocaleTable = LocaleTable {
    planets: PLANETS,
    signs: SIGNS,
    houses: HOUSES,
    aspects: ASPECTS,
    system: "Целые знаки",
    zodiac: "Тропический",
    anonymous: "Без имени",
    pdf: PdfChrome {
        nativity_of: "Натальная карта",
        prepared_by: "Подготовил",
        index_of_elements: "Указатель элементов",
        commentary: "Комментарий",
    },
};

static PLANETS: &[Entry] = &[
    Entry { slug: "sun", name: "Солнце", terms: &["солнце", "солнца", "солнцу", "солнцем"] },
    Entry { slug: "moon", name: "Луна", terms: &["луна", "луны", "луне", "луну", "луной"] },
    Entry {
        slug: "mercury",
        name: "Меркурий",
        terms: &["меркурий", "меркурия", "меркурию", "меркурием"],
    },
    Entry {
        slug: "venus",
        name: "Венера",
        terms: &["венера", "венеры", "венере", "венеру", "венерой"],
    },
    Entry { slug: "mars", name: "Марс", terms: &["марс", "марса", "марсу", "марсом", "марсе"] },
    Entry {
        slug: "jupiter",
        name: "Юпитер",
        terms: &["юпитер", "юпитера", "юпитеру", "юпитером"],
    },
    Entry {
        slug: "saturn",
        name: "Сатурн",
        terms: &["сатурн", "сатурна", "сатурну", "сатурном"],
    },
    Entry { slug: "uranus", name: "Уран", terms: &["уран", "урана", "урану", "ураном"] },
    Entry {
        slug: "neptune",
        name: "Нептун",
        terms: &["нептун", "нептуна", "нептуну", "нептуном"],
    },
    Entry {
        slug: "pluto",
        name: "Плутон",
        terms: &["плутон", "плутона", "плутону", "плутоном"],
    },
    Entry {
        slug: "ascendant",
        name: "Асцендент",
        terms: &["асцендент", "асцендента", "асценденту", "восходящий", "восходящего", "аск"],
    },
];

static SIGNS: &[Entry] = &[
    Entry { slug: "aries", name: "Овен", terms: &["овен", "овна", "овну", "овне", "овном"] },
    Entry {
        slug: "taurus",
        name: "Телец",
        terms: &["телец", "тельца", "тельцу", "тельце", "тельцом"],
    },
    Entry {
        slug: "gemini",
        name: "Близнецы",
        terms: &["близнецы", "близнецов", "близнецах", "близнецам"],
    },
    Entry { slug: "cancer", name: "Рак", terms: &["рак", "рака", "раку", "раке", "раком"] },
    Entry { slug: "leo", name: "Лев", terms: &["лев", "льва", "льву", "льве", "львом"] },
    Entry { slug: "virgo", name: "Дева", terms: &["дева", "девы", "деве", "деву", "девой"] },
    Entry { slug: "libra", name: "Весы", terms: &["весы", "весов", "весах", "весам"] },
    Entry {
        slug: "scorpio",
        name: "Скорпион",
        terms: &["скорпион", "скорпиона", "скорпиону", "скорпионе", "скорпионом"],
    },
    Entry {
        slug: "sagittarius",
        name: "Стрелец",
        terms: &["стрелец", "стрельца", "стрельцу", "стрельце", "стрельцом"],
    },
    Entry {
        slug: "capricorn",
        name: "Козерог",
        terms: &["козерог", "козерога", "козерогу", "козероге", "козерогом"],
    },
    Entry {
        slug: "aquarius",
        name: "Водолей",
        terms: &["водолей", "водолея", "водолею", "водолее", "водолеем"],
    },
    Entry { slug: "pisces", name: "Рыбы", terms: &["рыбы", "рыб", "рыбах", "рыбам"] },
];

// "в пятом доме" (prepositional) and "пятый дом" (nominative) both appear in
// speech; numeric forms catch "5 дом" / "дом 5".
static HOUSES: &[HouseEntry] = &[
    HouseEntry {
        name: "Первый дом",
        terms: &["первый дом", "первом доме", "1 дом", "1-й дом", "дом 1"],
    },
    HouseEntry {
        name: "Второй дом",
        terms: &["второй дом", "втором доме", "2 дом", "2-й дом", "дом 2"],
    },
    HouseEntry {
        name: "Третий дом",
        terms: &["третий дом", "третьем доме", "3 дом", "3-й дом", "дом 3"],
    },
    HouseEntry {
        name: "Четвёртый дом",
        terms: &[
            "четвёртый дом",
            "четвертый дом",
            "четвёртом доме",
            "четвертом доме",
            "4 дом",
            "4-й дом",
            "дом 4",
        ],
    },
    HouseEntry {
        name: "Пятый дом",
        terms: &["пятый дом", "пятом доме", "5 дом", "5-й дом", "дом 5"],
    },
    HouseEntry {
        name: "Шестой дом",
        terms: &["шестой дом", "шестом доме", "6 дом", "6-й дом", "дом 6"],
    },
    HouseEntry {
        name: "Седьмой дом",
        terms: &["седьмой дом", "седьмом доме", "7 дом", "7-й дом", "дом 7"],
    },
    HouseEntry {
        name: "Восьмой дом",
        terms: &["восьмой дом", "восьмом доме", "8 дом", "8-й дом", "дом 8"],
    },
    HouseEntry {
        name: "Девятый дом",
        terms: &["девятый дом", "девятом доме", "9 дом", "9-й дом", "дом 9"],
    },
    HouseEntry {
        name: "Десятый дом",
        terms: &["десятый дом", "десятом доме", "10 дом", "10-й дом", "дом 10"],
    },
    HouseEntry {
        name: "Одиннадцатый дом",
        terms: &["одиннадцатый дом", "одиннадцатом доме", "11 дом", "11-й дом", "дом 11"],
    },
    HouseEntry {
        name: "Двенадцатый дом",
        terms: &["двенадцатый дом", "двенадцатом доме", "12 дом", "12-й дом", "дом 12"],
    },
];

static ASPECTS: &[AspectEntry] = &[
    AspectEntry {
        kind: "conjunction",
        word: "соединение",
        match_words: &["соединение", "соединении", "соединения", "соединён", "соединен"],
    },
    AspectEntry {
        kind: "sextile",
        word: "секстиль",
        match_words: &["секстиль", "секстиле", "секстиля", "секстилем"],
    },
    AspectEntry {
        kind: "square",
        word: "квадрат",
        match_words: &["квадрат", "квадрате", "квадратом", "квадратура", "квадратуре", "квадратуры"],
    },
    AspectEntry {
        kind: "trine",
        word: "тригон",
        match_words: &["тригон", "тригоне", "тригоном", "трин", "трине"],
    },
    AspectEntry {
        kind: "opposition",
        word: "оппозиция",
        match_words: &["оппозиция", "оппозиции", "оппозицию", "оппозицией"],
    },
];
