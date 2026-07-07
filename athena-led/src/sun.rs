// ==========================================
// 🌅 sun.rs — 日出日落模块 (v2.4.0 新增)
// NOAA/《Almanac for Computers》标准天文公式，纯本地计算零网络。
// 经纬度来源: 手动参数 "lat,lon" 或 net_agent 的 IP 定位。
// 屏显格式: "6:02~19:23" (日出~日落)
// ==========================================

const D2R: f64 = std::f64::consts::PI / 180.0;

/// 计算日出或日落的 UTC 时刻 (小时, 0~24)。极昼/极夜返回 None
fn calc_event_utc(day_of_year: f64, lat: f64, lon: f64, rising: bool) -> Option<f64> {
    // 官方民用日出日落天顶角 (含大气折射修正)
    let zenith: f64 = 90.833;

    // 1. 经度换算小时 + 近似事件时刻
    let lng_hour = lon / 15.0;
    let t = if rising {
        day_of_year + ((6.0 - lng_hour) / 24.0)
    } else {
        day_of_year + ((18.0 - lng_hour) / 24.0)
    };

    // 2. 太阳平近点角
    let m = (0.9856 * t) - 3.289;

    // 3. 太阳真黄经
    let mut l = m + (1.916 * (m * D2R).sin()) + (0.020 * (2.0 * m * D2R).sin()) + 282.634;
    l = l.rem_euclid(360.0);

    // 4. 太阳赤经 (调整到与黄经同象限)
    let mut ra = ((0.91764 * (l * D2R).tan()).atan()) / D2R;
    ra = ra.rem_euclid(360.0);
    let l_quadrant = (l / 90.0).floor() * 90.0;
    let ra_quadrant = (ra / 90.0).floor() * 90.0;
    ra = (ra + (l_quadrant - ra_quadrant)) / 15.0;

    // 5. 太阳赤纬
    let sin_dec = 0.39782 * (l * D2R).sin();
    let cos_dec = sin_dec.asin().cos();

    // 6. 太阳时角
    let cos_h = ((zenith * D2R).cos() - (sin_dec * (lat * D2R).sin()))
        / (cos_dec * (lat * D2R).cos());
    if !(-1.0..=1.0).contains(&cos_h) {
        return None; // 极昼 (cos_h < -1) 或极夜 (cos_h > 1)
    }

    let h = if rising {
        360.0 - cos_h.acos() / D2R
    } else {
        cos_h.acos() / D2R
    } / 15.0;

    // 7. 本地平太阳时 -> UTC
    let t_local = h + ra - (0.06571 * t) - 6.622;
    Some((t_local - lng_hour).rem_euclid(24.0))
}

// 小时 (f64) -> "H:MM"
fn fmt_hour(hours: f64) -> String {
    let total_min = (hours * 60.0).round() as i64 % (24 * 60);
    format!("{}:{:02}", total_min / 60, total_min % 60)
}

/// 生成屏显字符串: "6:02~19:23"。tz_hours = 本地时区偏移 (东八区 = 8.0)
pub fn sun_string(lat: f64, lon: f64, day_of_year: u32, tz_hours: f64) -> String {
    let doy = day_of_year as f64;
    match (
        calc_event_utc(doy, lat, lon, true),
        calc_event_utc(doy, lat, lon, false),
    ) {
        (Some(rise_utc), Some(set_utc)) => {
            let rise = (rise_utc + tz_hours).rem_euclid(24.0);
            let set = (set_utc + tz_hours).rem_euclid(24.0);
            format!("{}~{}", fmt_hour(rise), fmt_hour(set))
        }
        // 高纬度极昼/极夜
        (None, _) | (_, None) => "SUN:--".to_string(),
    }
}

/// 便捷入口: 今天 + 本机时区
pub fn today_string(lat: f64, lon: f64) -> String {
    use chrono::{Datelike, Local, Offset};
    let now = Local::now();
    let tz_hours = now.offset().fix().local_minus_utc() as f64 / 3600.0;
    sun_string(lat, lon, now.ordinal(), tz_hours)
}

/// 解析模块参数 "lat,lon" (如 "39.90,116.40")
pub fn parse_coords(param: &str) -> Option<(f64, f64)> {
    let (lat_s, lon_s) = param.trim().split_once(',')?;
    let lat: f64 = lat_s.trim().parse().ok()?;
    let lon: f64 = lon_s.trim().parse().ok()?;
    if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
        Some((lat, lon))
    } else {
        None
    }
}

// ==========================================
// 🧪 单元测试: 用公开可查的北京日出日落时间校验 (允许 ±15 分钟)
// ==========================================
#[cfg(test)]
mod tests {
    use super::*;

    // "H:MM" -> 分钟数
    fn to_min(s: &str) -> i64 {
        let (h, m) = s.split_once(':').unwrap();
        h.parse::<i64>().unwrap() * 60 + m.parse::<i64>().unwrap()
    }

    #[test]
    fn beijing_summer_solstice() {
        // 北京 (39.90N, 116.40E) 2025-06-21 (第172天):
        // 日出约 04:46, 日落约 19:46 (东八区)
        let s = sun_string(39.90, 116.40, 172, 8.0);
        let (rise, set) = s.split_once('~').unwrap();
        assert!((to_min(rise) - to_min("4:46")).abs() <= 15, "rise={}", rise);
        assert!((to_min(set) - to_min("19:46")).abs() <= 15, "set={}", set);
    }

    #[test]
    fn beijing_winter_solstice() {
        // 北京 2025-12-21 (第355天): 日出约 07:33, 日落约 16:53
        let s = sun_string(39.90, 116.40, 355, 8.0);
        let (rise, set) = s.split_once('~').unwrap();
        assert!((to_min(rise) - to_min("7:33")).abs() <= 15, "rise={}", rise);
        assert!((to_min(set) - to_min("16:53")).abs() <= 15, "set={}", set);
    }

    #[test]
    fn polar_night() {
        // 北极圈内冬至应返回极夜占位
        assert_eq!(sun_string(78.0, 15.0, 355, 1.0), "SUN:--");
    }

    #[test]
    fn coords_parsing() {
        assert_eq!(parse_coords("39.90,116.40"), Some((39.90, 116.40)));
        assert_eq!(parse_coords(" 31.2 , 121.5 "), Some((31.2, 121.5)));
        assert_eq!(parse_coords("999,0"), None);
        assert_eq!(parse_coords("abc"), None);
    }
}
