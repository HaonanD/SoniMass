library(tuneR)

#' 合成并保存音频函数
#'
#' @param wave_df 数据框。第一列必须是频率(numeric)，第二列必须是振幅函数列表(list of functions)。
#' @param duration 音频时长 (秒)。
#' @param file_name 输出的文件名 (例如 "output.wav")。
#' @param samp_rate 采样率，默认为 44100。
#'
#' @return 返回生成的 Wave 对象 (同时会在磁盘写入文件)
library(tuneR)

synth_from_df <- function(wave_df, duration, file_name, samp_rate = 44100) {
  
  # 1. 生成时间轴
  t <- seq(0, duration, by = 1/samp_rate)
  
  # 2. 初始化
  final_sound <- numeric(length(t))
  n_waves <- nrow(wave_df)
  
  # 3. 叠加波形
  for (i in 1:n_waves) {
    f <- wave_df[i, 1]
    amp_func <- wave_df[[2]][[i]] # 提取函数
    
    # 叠加
    final_sound <- final_sound + (amp_func(t) * sin(2 * pi * f * t))
  }
  
  # 4. 创建初步的 Wave 对象
  # 此时 final_sound 是浮点数，数值可能很大（比如叠加后变成了 5.0），也可能很小
  # bit=16 只是告诉 R 我们打算用 16位精度，但数据暂时还是 float
  w_obj <- Wave(final_sound, samp.rate = samp_rate, bit = 16)
  
  # 5. 【关键步骤】使用 normalize
  # unit = "16": 强制转换为 16-bit 整数范围 (-32767 到 32767)
  # level = 1: 使用 100% 的音量范围 (0dBFS)
  # rescale = TRUE: 自动拉伸波形到最大
  w_obj_norm <- normalize(w_obj, unit = "16") 
  
  # 6. 保存
  writeWave(w_obj_norm, file_name)
  
  message(paste("保存成功:", file_name))
  return(w_obj_norm)
}

if (FALSE) {
  # --- 准备数据 ---

  # 定义两个振幅函数
  func_decay <- function(t) { exp(-3 * t) }             # 指数衰减
  func_tremolo <- function(t) { (sin(2*pi*5*t) + 1)/4 } # 5Hz 颤音
  func_norm <- function(t) { dnorm(t, mean = 2.35, sd = 1) } #正态分布函数
  make_norm_func <- function(a) {
    # 返回一个函数，这个函数只接受 t
    function(t) {
      a * dnorm(t, mean = 2.35, sd = 1)
    }
  }

  # 创建数据框
  # 注意：在普通 data.frame 中放入 list，最好加上 I() 保护，或者使用 list()
  # 这里的结构：第一列 numeric，第二列 list
  my_data <- data.frame(
    freq = c(440, 441, 442, 443, 444, 445), # 拍频
    amp_f = I(list(make_norm_func(1), make_norm_func(0.5), make_norm_func(0.25), make_norm_func(0.125), make_norm_func(0.0625), make_norm_func(0.03125))) # 使用 I() 确保被视为一列 list
  )

  # 检查一下数据结构
  # str(my_data)

  # --- 调用函数 ---
  # 只需要一行代码
  result_wave <- synth_from_df(
    wave_df = my_data,
    duration = 5,
    file_name = "My_Synthesized_Sound.wav",
    samp_rate = 44100
  )

  # --- 可选：画图看看结果 ---
  png("My_Synthesized_Sound.png")
  plot(result_wave, main = "合成结果波形")
  dev.off()
  
}


f1 <- function(x) {
  440*2^(x/12)
}

f1(-2)/
f1(-9)

func_norm(0.5)

FWHM <- function(sd) { 2*sqrt(2*log(2))*sd }
FWHM(1)
