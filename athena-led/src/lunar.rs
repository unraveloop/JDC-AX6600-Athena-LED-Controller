// ==========================================
// 🏮 lunar.rs — 农历日期模块 (v2.4.0 新增)
// 经典查表算法，覆盖 1900~2100 年，零依赖零网络。
// 点阵屏无法显示汉字，采用数字表达: "L:5.7" = 五月初七,
// 闰月加 R 前缀: "L:R6.15" = 闰六月十五
// ==========================================
use chrono::{Datelike, NaiveDate};

// 1900-2100 年农历数据表 (每年一个 u32):
//   bits 0-3   : 闰月月份 (0 = 无闰月)
//   bits 4-15  : 12 个月的大小月 (0x8000>>月序, 1=30天大月, 0=29天小月)
//   bit  16    : 闰月天数 (1=30天, 0=29天)
// 该表为社区标准表 (最早出自香港天文台数据)，单元测试锚定多个已知
// 春节/中秋/闰月做正确性校验。
const LUNAR_INFO: [u32; 201] = [
    0x04bd8, 0x04ae0, 0x0a570, 0x054d5, 0x0d260, 0x0d950, 0x16554, 0x056a0, 0x09ad0, 0x055d2, // 1900-1909
    0x04ae0, 0x0a5b6, 0x0a4d0, 0x0d250, 0x1d255, 0x0b540, 0x0d6a0, 0x0ada2, 0x095b0, 0x14977, // 1910-1919
    0x04970, 0x0a4b0, 0x0b4b5, 0x06a50, 0x06d40, 0x1ab54, 0x02b60, 0x09570, 0x052f2, 0x04970, // 1920-1929
    0x06566, 0x0d4a0, 0x0ea50, 0x06e95, 0x05ad0, 0x02b60, 0x186e3, 0x092e0, 0x1c8d7, 0x0c950, // 1930-1939
    0x0d4a0, 0x1d8a6, 0x0b550, 0x056a0, 0x1a5b4, 0x025d0, 0x092d0, 0x0d2b2, 0x0a950, 0x0b557, // 1940-1949
    0x06ca0, 0x0b550, 0x15355, 0x04da0, 0x0a5b0, 0x14573, 0x052b0, 0x0a9a8, 0x0e950, 0x06aa0, // 1950-1959
    0x0aea6, 0x0ab50, 0x04b60, 0x0aae4, 0x0a570, 0x05260, 0x0f263, 0x0d950, 0x05b57, 0x056a0, // 1960-1969
    0x096d0, 0x04dd5, 0x04ad0, 0x0a4d0, 0x0d4d4, 0x0d250, 0x0d558, 0x0b540, 0x0b6a0, 0x195a6, // 1970-1979
    0x095b0, 0x049b0, 0x0a974, 0x0a4b0, 0x0b27a, 0x06a50, 0x06d40, 0x0af46, 0x0ab60, 0x09570, // 1980-1989
    0x04af5, 0x04970, 0x064b0, 0x074a3, 0x0ea50, 0x06b58, 0x055c0, 0x0ab60, 0x096d5, 0x092e0, // 1990-1999
    0x0c960, 0x0d954, 0x0d4a0, 0x0da50, 0x07552, 0x056a0, 0x0abb7, 0x025d0, 0x092d0, 0x0cab5, // 2000-2009
    0x0a950, 0x0b4a0, 0x0baa4, 0x0ad50, 0x055d9, 0x04ba0, 0x0a5b0, 0x15176, 0x052b0, 0x0a930, // 2010-2019
    0x07954, 0x06aa0, 0x0ad50, 0x05b52, 0x04b60, 0x0a6e6, 0x0a4e0, 0x0d260, 0x0ea65, 0x0d530, // 2020-2029
    0x05aa0, 0x076a3, 0x096d0, 0x04afb, 0x04ad0, 0x0a4d0, 0x1d0b6, 0x0d250, 0x0d520, 0x0dd45, // 2030-2039
    0x0b5a0, 0x056d0, 0x055b2, 0x049b0, 0x0a577, 0x0a4b0, 0x0aa50, 0x1b255, 0x06d20, 0x0ada0, // 2040-2049
    0x14b63, 0x09370, 0x049f8, 0x04970, 0x064b0, 0x168a6, 0x0ea50, 0x06b20, 0x1a6c4, 0x0aae0, // 2050-2059
    0x0a2e0, 0x0d2e3, 0x0c960, 0x0d557, 0x0d4a0, 0x0da50, 0x05d55, 0x056a0, 0x0a6d0, 0x055d4, // 2060-2069
    0x052d0, 0x0a9b8, 0x0a950, 0x0b4a0, 0x0b6a6, 0x0ad50, 0x055a0, 0x0aba4, 0x0a5b0, 0x052b0, // 2070-2079
    0x0b273, 0x06930, 0x07337, 0x06aa0, 0x0ad50, 0x14b55, 0x04b60, 0x0a570, 0x054e4, 0x0d160, // 2080-2089
    0x0e968, 0x0d520, 0x0daa0, 0x16aa6, 0x056d0, 0x04ae0, 0x0a9d4, 0x0a2d0, 0x0d150, 0x0f252, // 2090-2099
    0x0d520,                                                                                    // 2100
];

// 某年的闰月月份 (0 = 无)
fn leap_month(year: i32) -> u32 {
    LUNAR_INFO[(year - 1900) as usize] & 0xf
}

// 某年闰月的天数 (无闰月 = 0)
fn leap_days(year: i32) -> u32 {
    if leap_month(year) != 0 {
        if LUNAR_INFO[(year - 1900) as usize] & 0x10000 != 0 { 30 } else { 29 }
    } else {
        0
    }
}

// 某年第 m 个普通月的天数 (m: 1~12)
fn month_days(year: i32, m: u32) -> u32 {
    if LUNAR_INFO[(year - 1900) as usize] & (0x10000 >> m) != 0 { 30 } else { 29 }
}

// 某农历年总天数
fn year_days(year: i32) -> u32 {
    let mut sum = 348; // 12 × 29
    let info = LUNAR_INFO[(year - 1900) as usize];
    let mut mask = 0x8000;
    while mask > 0x8 {
        if info & mask != 0 { sum += 1; }
        mask >>= 1;
    }
    sum + leap_days(year)
}

/// 公历 -> 农历 (年, 月, 日, 是否闰月)。超出 1900~2100 范围返回 None
pub fn to_lunar(date: NaiveDate) -> Option<(i32, u32, u32, bool)> {
    // 农历纪元锚点: 1900-01-31 = 农历 1900 年正月初一
    let base = NaiveDate::from_ymd_opt(1900, 1, 31)?;
    let mut offset = (date - base).num_days();
    if offset < 0 {
        return None;
    }

    // 1. 逐年扣减
    let mut year = 1900;
    while year <= 2100 {
        let days = year_days(year) as i64;
        if offset < days { break; }
        offset -= days;
        year += 1;
    }
    if year > 2100 {
        return None;
    }

    // 2. 逐月扣减 (闰月紧跟在其名义月之后)
    let leap = leap_month(year);
    let mut is_leap = false;
    let mut month = 1u32;
    loop {
        let days = if is_leap {
            leap_days(year) as i64
        } else {
            month_days(year, month) as i64
        };

        if offset < days {
            break;
        }
        offset -= days;

        if is_leap {
            // 闰月结束，进入下一个普通月
            is_leap = false;
            month += 1;
        } else if leap != 0 && month == leap {
            // 下一个是闰月 (月序不变)
            is_leap = true;
        } else {
            month += 1;
        }

        if month > 12 {
            // 理论上不会到这里 (年天数已对齐)，防御性退出
            return None;
        }
    }

    Some((year, month, (offset + 1) as u32, is_leap))
}

/// 屏显格式: "L:5.7" (五月初七) / "L:R6.15" (闰六月十五)
pub fn lunar_string(date: NaiveDate) -> String {
    match to_lunar(date) {
        Some((_, month, day, is_leap)) => {
            if is_leap {
                format!("L:R{}.{}", month, day)
            } else {
                format!("L:{}.{}", month, day)
            }
        }
        None => "L:Err".to_string(),
    }
}

/// 便捷入口: 今天的农历
pub fn today_string() -> String {
    lunar_string(chrono::Local::now().date_naive())
}

// 防止公历部分 (Datelike) 被误报未使用
#[allow(dead_code)]
fn _unused(d: NaiveDate) -> i32 { d.year() }

// ==========================================
// 🧪 单元测试: 用公开可查的春节/中秋/闰月锚点校验数据表
// ==========================================
#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn spring_festivals() {
        // 各年春节 (正月初一) 公历日期
        assert_eq!(to_lunar(d(2020, 1, 25)), Some((2020, 1, 1, false)));
        assert_eq!(to_lunar(d(2021, 2, 12)), Some((2021, 1, 1, false)));
        assert_eq!(to_lunar(d(2022, 2, 1)),  Some((2022, 1, 1, false)));
        assert_eq!(to_lunar(d(2023, 1, 22)), Some((2023, 1, 1, false)));
        assert_eq!(to_lunar(d(2024, 2, 10)), Some((2024, 1, 1, false)));
        assert_eq!(to_lunar(d(2025, 1, 29)), Some((2025, 1, 1, false)));
        assert_eq!(to_lunar(d(2026, 2, 17)), Some((2026, 1, 1, false)));
    }

    #[test]
    fn mid_autumn_and_duanwu() {
        // 2024-09-17 中秋 (八月十五)
        assert_eq!(to_lunar(d(2024, 9, 17)), Some((2024, 8, 15, false)));
        // 2023-06-22 端午 (五月初五)
        assert_eq!(to_lunar(d(2023, 6, 22)), Some((2023, 5, 5, false)));
        // 2025-10-06 中秋 (八月十五)
        assert_eq!(to_lunar(d(2025, 10, 6)), Some((2025, 8, 15, false)));
    }

    #[test]
    fn leap_months() {
        // 已知闰月年份: 2020 闰四月, 2023 闰二月, 2025 闰六月
        assert_eq!(leap_month(2020), 4);
        assert_eq!(leap_month(2023), 2);
        assert_eq!(leap_month(2025), 6);
        assert_eq!(leap_month(2024), 0);
    }

    #[test]
    fn epoch_anchor() {
        // 纪元锚点: 1900-01-31 = 1900 年正月初一
        assert_eq!(to_lunar(d(1900, 1, 31)), Some((1900, 1, 1, false)));
    }

    #[test]
    fn display_format() {
        assert_eq!(lunar_string(d(2024, 2, 10)), "L:1.1");
        // 2025 闰六月期间的某天应带 R 前缀 (闰六月初一 = 2025-07-25)
        assert_eq!(lunar_string(d(2025, 7, 25)), "L:R6.1");
    }
}
