use anyhow::Result;
use sysfs_gpio::{Direction, Pin};
use crate::char_dict::CHAR_DICT;

const LOW: u8 = 0x00;
const HIGH: u8 = 0x01;

// Display mode commands
const COMMAND1: u8 = 0b00000011; // Display mode
const COMMAND2: u8 = 0b01000000; // Data mode
const COMMAND3: u8 = 0b11000000; // Display address

pub struct LedScreen {
    left_screen: LedScreenUnit,
    right_screen: LedScreenUnit,
}

pub struct LedScreenUnit {
    stb: Pin,
    clk: Pin,
    dio: Pin,
}

impl LedScreen {
    pub fn new(stb_left: u64, stb_right: u64, clk: u64, dio: u64) -> Result<Self> {
        let left_screen = LedScreenUnit::new(stb_left, clk, dio)?;
        let right_screen = LedScreenUnit::new(stb_right, clk, dio)?;
        
        let mut screen = Self {
            left_screen,
            right_screen,
        };
        
        screen.set_show_model()?;
        screen.set_data_model()?;
        
        Ok(screen)
    }

    pub fn set_show_model(&mut self) -> Result<()> {
        self.left_screen.set_show_model()?;
        self.right_screen.set_show_model()?;
        Ok(())
    }

    pub fn set_data_model(&mut self) -> Result<()> {
        self.left_screen.set_data_model()?;
        self.right_screen.set_data_model()?;
        Ok(())
    }

    pub fn power(&mut self, run: bool, light_level: u8) -> Result<()> {
        self.left_screen.power(run, light_level)?;
        self.right_screen.power(run, light_level)?;
        Ok(())
    }

pub fn write_data(&mut self, text: &[u8], status: u8) -> Result<()> {
        let mut display_data = Vec::new();
        
        // [核心修复] 
        // 1. 尝试把字节流转成 UTF-8 字符串
        // 2. 按【字符】(chars) 遍历，而不是按字节
        // 这样 '℃'、'☀' 这种多字节符号才能被正确识别！
        let content = std::str::from_utf8(text).unwrap_or("");
        
        for ch in content.chars() {
            // 统一转大写匹配 (兼容 a-z)
            let key = ch.to_ascii_uppercase(); 
            
            if let Some(bytes) = CHAR_DICT.get(&key) {
                display_data.extend_from_slice(bytes);
                // 统一加 1 列间距
                display_data.push(0x00); 
            }
        }

        if display_data.len() > 27 {
            self.flow(&display_data, status)?;
        } else {
            self.static_display(&display_data, status)?;
        }
        Ok(())
    }

    fn flow(&mut self, data: &[u8], status: u8) -> Result<()> {
        let mut start = 0;
        for i in 1..=data.len() {
            let mut off = [0u8; 27];
            if i > 27 {
                start += 1;
            }
            off[..i.min(27)].copy_from_slice(&data[start..start + i.min(27)]);
            self.do_write_data(&off, status)?;
            std::thread::sleep(std::time::Duration::from_millis(128));
        }
        Ok(())
    }

    fn static_display(&mut self, data: &[u8], status: u8) -> Result<()> {
        let mut display_data = [0u8; 27];
        if data.len() < 27 {
            let offset = (27 - data.len()) / 2;
            display_data[offset..offset + data.len()].copy_from_slice(data);
        } else {
            display_data[..27].copy_from_slice(&data[..27]);
        }
        self.do_write_data(&display_data, status)?;
        Ok(())
    }

    fn do_write_data(&mut self, values: &[u8], status: u8) -> Result<()> {
        self.left_screen.printf(&values[..14])?;
        let mut right_data = values[14..27].to_vec();
        right_data.push(status);
        self.right_screen.printf(&right_data)?;
        Ok(())
    }
}

impl LedScreenUnit {
    fn new(stb: u64, clk: u64, dio: u64) -> Result<Self> {
        let stb_pin = Pin::new(stb);
        let clk_pin = Pin::new(clk);
        let dio_pin = Pin::new(dio);

        stb_pin.export()?;
        clk_pin.export()?;
        dio_pin.export()?;

        stb_pin.set_direction(Direction::Out)?;
        clk_pin.set_direction(Direction::Out)?;
        dio_pin.set_direction(Direction::Out)?;

        Ok(Self {
            stb: stb_pin,
            clk: clk_pin,
            dio: dio_pin,
        })
    }

    fn set_show_model(&mut self) -> Result<()> {
        self.do_write_data(COMMAND1, &[])?;
        Ok(())
    }

    fn set_data_model(&mut self) -> Result<()> {
        self.do_write_data(COMMAND2, &[])?;
        Ok(())
    }

    fn power(&mut self, run: bool, light_level: u8) -> Result<()> {
        let command = if run {
            (light_level << 5 >> 5 | 0b11111000) & 0b10001111
        } else {
            0b10000000
        };
        self.do_write_data(command, &[])?;
        Ok(())
    }

    fn printf(&mut self, values: &[u8]) -> Result<()> {
        self.do_write_data(COMMAND3, values)?;
        Ok(())
    }

    fn do_write_data(&mut self, command: u8, values: &[u8]) -> Result<()> {
        self.stb.set_value(LOW)?;
        self.write_command_byte(command)?;
        
        for (i, &value) in values.iter().enumerate() {
            self.write_data_byte(value, i % 2 != 0)?;
        }
        
        self.stb.set_value(HIGH)?;
        Ok(())
    }

    fn write_command_byte(&mut self, value: u8) -> Result<()> {
        for i in 0..8 {
            let bit = (value >> i) & 0x01;
            self.write_bit(bit)?;
        }
        Ok(())
    }

    fn write_data_byte(&mut self, value: u8, fill_data: bool) -> Result<()> {
        for i in 0..5 {
            let bit = (value >> i) & 0x01;
            self.write_bit(bit)?;
        }
        
        if fill_data {
            for _ in 0..6 {
                self.write_bit(LOW)?;
            }
        }
        Ok(())
    }

    fn write_bit(&mut self, bit: u8) -> Result<()> {
        self.clk.set_value(LOW)?;
        self.dio.set_value(bit)?;
        self.clk.set_value(HIGH)?;
        Ok(())
    }
}

impl Drop for LedScreenUnit {
    fn drop(&mut self) {
        let _ = self.stb.unexport();
        let _ = self.clk.unexport();
        let _ = self.dio.unexport();
    }
}
