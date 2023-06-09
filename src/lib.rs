#![no_std]

use embedded_hal_1::i2c::I2c;

pub const DEFAULT_ADDR: u8 = 0x55;

pub mod regs {
    pub const STATUS: u8 = 0x01;
    pub const CONTACT_COUNT_MAX: u8 = 0x3F;
    pub const MISC_INFO: u8 = 0xF0;

    pub const XY_RESOLUTION_H: u8 = 0x04;
    pub const X_RESOLUTION_L: u8 = 0x05;
    pub const Y_RESOLUTION_L: u8 = 0x06;

    pub const SENSING_COUNTER_L: u8 = 0x07;
    pub const SENSING_COUNTER_H: u8 = 0x08;

    pub const ADVANCED_TOUCH_INFO: u8 = 0x10;
}

pub struct TouchIC<I2C> {
    i2c: I2C,
    addr: u8,
}

impl<I2C> TouchIC<I2C>
where
    I2C: I2c,
{
    pub fn new(i2c: I2C, addr: u8) -> Self {
        Self { i2c, addr }
    }

    pub fn new_default(i2c: I2C) -> Self {
        Self::new(i2c, DEFAULT_ADDR)
    }

    pub fn init(&mut self) -> Result<(), I2C::Error> {
        self.wait_normal_status()?;
        Ok(())
    }

    pub fn get_gesture_info(&mut self) -> Result<GestureInfo, I2C::Error> {
        let raw = self.read_reg8(regs::ADVANCED_TOUCH_INFO)?;
        Ok(GestureInfo {
            gesture_type: GestureType::from_u8(raw),
            proximity: raw & 0b0100_0000 != 0,
            water: raw & 0b0010_0000 != 0,
        })
    }

    pub fn get_point0(&mut self) -> Result<Option<Point>, I2C::Error> {
        self.get_point(0)
    }

    pub fn get_point1(&mut self) -> Result<Option<Point>, I2C::Error> {
        self.get_point(1)
    }

    pub fn get_point(&mut self, nth: u8) -> Result<Option<Point>, I2C::Error> {
        if nth > 9 {
            return Ok(None); // max 10 points
        }
        let start_reg = 0x12 + 4 * nth;
        let mut buf = [0u8; 4];
        self.i2c.write_read(self.addr, &[start_reg], &mut buf)?;

        if buf[0] >> 7 == 0 {
            return Ok(None);
        } else {
            let x = (u16::from(buf[0] & 0b0111_0000) << 4) | u16::from(buf[1]);
            let y = (u16::from(buf[0] & 0b0000_1111) << 8) | u16::from(buf[2]);
            Ok(Some(Point { x, y }))
        }
    }

    /// Sensing Counter Registers provide a frame-based scan counter for host to verify current scan rate.
    pub fn get_sensor_count(&mut self) -> Result<u16, I2C::Error> {
        let mut buf = [0u8; 2];
        self.i2c
            .write_read(self.addr, &[regs::SENSING_COUNTER_L], &mut buf)?;

        Ok(u16::from_be_bytes(buf))
    }

    pub fn get_capabilities(&mut self) -> Result<Capabilities, I2C::Error> {
        let max_contacts = self.read_reg8(regs::CONTACT_COUNT_MAX)?;
        let misc_info = self.read_reg8(regs::MISC_INFO)?;

        let mut buf = [0u8; 3];
        self.i2c
            .write_read(self.addr, &[regs::XY_RESOLUTION_H], &mut buf)?;

        let x_res = ((u16::from(buf[0]) & 0b0111_0000) << 4) | u16::from(buf[1]);
        let y_res = ((u16::from(buf[0]) & 0b0000_1111) << 8) | u16::from(buf[2]);

        Ok(Capabilities {
            max_touches: max_contacts,
            max_x: x_res,
            max_y: y_res,
            smart_wake_up: misc_info & 0b1000_0000 != 0,
        })
    }

    fn wait_normal_status(&mut self) -> Result<(), I2C::Error> {
        let mut status = self.read_reg8(regs::STATUS)?;
        while status & 0xf0 != 0 {
            status = self.read_reg8(regs::STATUS)?;
        }
        Ok(())
    }

    fn read_reg8(&mut self, reg: u8) -> Result<u8, I2C::Error> {
        let mut buf = [0u8; 1];
        self.i2c.write_read(self.addr, &[reg], &mut buf)?;
        Ok(buf[0])
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Capabilities {
    /// Maximum Number of Contacts Support Register
    pub max_touches: u8,
    // XY resolution
    pub max_x: u16,
    pub max_y: u16,
    pub smart_wake_up: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GestureInfo {
    pub gesture_type: GestureType,
    pub proximity: bool,
    pub water: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GestureType {
    None = 0,
    DoubleTab = 1,
    ZoomIn = 2,
    ZoomOut = 3,
    SlideLeftToRight = 4,
    SlideRightToLeft = 5,
    SlideTopToBottom = 6,
    SlideBottomToTop = 7,
    Palm = 8,
    SingleTap = 9,
    LongPress = 10,
    EndOfLongPress = 11,
    Drag = 12,
}

impl GestureType {
    fn from_u8(raw: u8) -> Self {
        match raw & 0x0f {
            0 => Self::None,
            1 => Self::DoubleTab,
            2 => Self::ZoomIn,
            3 => Self::ZoomOut,
            4 => Self::SlideLeftToRight,
            5 => Self::SlideRightToLeft,
            6 => Self::SlideTopToBottom,
            7 => Self::SlideBottomToTop,
            8 => Self::Palm,
            9 => Self::SingleTap,
            10 => Self::LongPress,
            11 => Self::EndOfLongPress,
            12 => Self::Drag,
            _ => unreachable!(),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Point {
    pub x: u16,
    pub y: u16,
}
