
use wasm_bindgen::prelude::*;
use crate::log;
use super::xtime;

#[wasm_bindgen]
pub struct FpsCounter {
    frame_count: i32,
    time_stamp : f64,
    fps : i32,
}

#[wasm_bindgen]
impl FpsCounter {
    
    pub fn new() -> FpsCounter {

        FpsCounter {
            frame_count : 0,
            time_stamp : xtime::now(),
            fps : 0,
        }
    }

    pub fn update(&mut self) {

        //更新帧率
        self.update_frame_calc()
    }

     //每隔1秒计算一次帧率
    fn update_frame_calc(&mut self) {

        self.frame_count = &self.frame_count + 1;

        let now = xtime::now();

        if now - self.time_stamp >= 1000.0 {
            
            let passed = now - self.time_stamp;
            self.fps = ((self.frame_count * 1000) as f64 / passed).round() as i32;

            log!("now:{} | {} | fps:{}",now, passed, self.fps);
            self.frame_count = 0;
            self.time_stamp = now;
        }
    }

    pub fn get_fps(&self) -> i32 {
        self.fps
    }
}


#[cfg(test)]
mod unit_test{
    
    #[test]
    fn test1(){
    }
}
