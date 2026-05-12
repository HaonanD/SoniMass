# Cicada

Cicada 是一款旨在将质谱 (Mass Spectrometry, MS) 原始数据转化为音频 (Sonification) 的高性能工具。通过将化学属性映射到声学参数，它为复杂质谱数据的探索和模式识别提供了一个全新的维度。

> **关于命名**：蝉 (Cicada) 的鸣叫是自然界中最壮观的“海量并发”。成千上万只蝉同时振动鼓膜，形成连续且宏大的共鸣。正如本项目将质谱中数以万计的离子信号转化为交织的正弦波，让沉默的数据如蝉鸣般发声。

## 基本原理

从数学本质上讲，复杂的声音可以看作是无数个单一频率正弦波的叠加。每一个正弦波都有自己随时间变化的振幅（强度）和特定的相位。

$$ f(t) = \sum_{n=1}^{\infty} A_n(t) \cdot \sin(2\pi F_n t + \phi_n) $$

为了简化计算，Cicada 将所有相位 $\phi_n$ 设定为 0。声音的特征由质谱数据与频率/振幅的映射关系决定：

- **频率 ($F_n$)**：映射自 **质荷比 (m/z)**。
- **振幅包络 ($A_n(t)$)**：类似于单个 m/z 的 **离子色谱图 (XIC)**。

### 音乐上的合理性
这种映射方式在音乐逻辑上具有天然的对应关系：
- **同位素峰**：M+1, M+2 等峰位会形成类似于“拍频”的声音效果。
- **电荷状态**：小分子或肽段的不同电荷状态（M/1, M/2, M/3）形成的结构类似于乐器的“泛音”。

## 工作流程 (Workflow)

Cicada **专注于处理质心化 (Centroided) 后的质谱数据**。为了将质谱数据转化为音频，它遵循以下四个核心步骤：

1. **信号追踪与归并**
   - 遍历全谱质心数据，应用“链式追踪”算法将相邻 Scan 中属于同一化合物的信号点合并为完整的轨迹链。
   
   > **针对不同采集模式的策略 (Data Strategy)**：
   > - **DDA (Data-Dependent Acquisition)**：仅处理 **MS1** 数据。由于 DDA 的 MS2 是随机触发的离散事件，缺乏时间上的连续性，无法生成平滑的音频，因此在生成时将被忽略。
   > - **DIA (Data-Independent Acquisition)**：采用 **"双唱片" (Dual Disc)** 模式。
   >   - **Disc 1 (MS1)**：处理母离子全谱，展现样品全貌。
   >   - **Disc 2 (MS2)**：由于 DIA 的 MS2 具有循环连续性，碎片离子同样拥有完整的色谱峰形。Cicada 将对其应用同样的 Hill Building 算法，将其转化为具有丰富**和弦 (Chords)** 细节的音频流。

2. **确定频率与振幅轨迹**
   - 针对每一条轨迹链，计算强度加权平均 m/z 以确定固定频率 $F_n$。
   - 提取该链在时间维度上的强度变化，形成离散的二维散点数据 $(Time, Intensity)$。
3. **生成连续振幅包络 $A_n(t)$**
   - 对离散的轨迹点应用 **PCHIP 插值**，生成与音频采样率（如 44.1kHz）对齐的连续振幅曲线。
4. **音频合成**
   - 将计算出的频率 $F_n$ 和连续振幅包络 $A_n(t)$ 代入正弦叠加公式，合成最终的音频波形。

## 技术实现要点

### 1. m/z → 频率：对数映射

Cicada 采用**对数映射**将 m/z 转化为频率，符合人类听觉的等程感知（等 m/z 区间对应等音程）：

$$ F = 30 \cdot \left(\frac{4200}{30}\right)^{\frac{m/z - 300}{1000 - 300}} $$

- 映射范围：`[300, 1000] m/z → [30, 4200] Hz`
- 中点 650 m/z → 几何平均值 ~354 Hz
- 超出范围的 m/z 值被 clamp 到边界

### 2. 强度 → 振幅：对数压缩

质谱数据的动态范围可达 5–6 个数量级。合成前对强度值进行对数压缩，使弱信号可听：

$$ A = \ln(1 + \text{intensity}) $$

此变换在 `Synthesizer::render` 中、构建 PCHIP 插值器前完成，不改动原始 Hill 数据。

### 3. 信号追踪：Hill Building

参考 Dinosaur（DOI: 10.1021/acs.jproteome.6b00016）的 Hill Building 逻辑，实现**双指针贪婪匹配**将连续 Scan 中 m/z 相近的峰连接为轨迹（Hill）：

- 时间复杂度：$O(N)$（利用 Scan 内 m/z 天然有序）
- `mz_guess` 滚动均值：抵抗质谱仪测量抖动，防止轨迹意外断裂
- Gap Skipping：默认允许跨越 1 个缺失 Scan 继续连接
- **不做任何过滤**：无峰分裂、无同位素打分、无电荷去卷积——保留所有信号

### 4. 振幅包络：PCHIP 插值

对每条 Hill 稀疏的 `(time, intensity)` 数据点应用 PCHIP（分段三次 Hermite 插值）生成连续振幅包络：

- **保形性**：严格保持单调性，不产生负强度的下冲（Undershoot）
- **平滑度**：一阶导数连续，消除线性插值的”折线”拉链噪音

### 5. 合成性能优化

| 优化 | 方法 | 收益 |
|------|------|------|
| 振幅采样 | 每 64 样本调用一次 PCHIP，中间线性插值 | PCHIP 调用量减少 ~64× |
| 相位推进 | sin 递推关系（每块仅 1 次 sin/cos 初始化） | 消除内层循环超越函数调用 |
| 并行合成 | Rayon 按 1 秒时间桶并行渲染 | 充分利用多核 |
| 插值器复用 | 所有 PCHIP 对象并行预构建，每 Hill 仅构建一次 | 消除重复构造开销 |

### 6. 可视化输出

每次运行默认生成两个配套可视化文件：

- **热图 PNG**（`*_heatmap.png`，1600×800）：将所有 Hill 光栅化为时间×m/z 二维图，强度以对数压缩后映射为 Plasma 色阶（深紫 → 洋红 → 橙色 → 亮黄）。
- **交互式 HTML 查看器**（`*.html`）：在浏览器中打开，热图叠加坐标轴（左 m/z、右 Hz、下 Time），并内嵌 WAV 播放器；播放时白色竖线实时同步标注当前时间位置。

可用 `--no-export-viz` 跳过可视化输出。

## 安装与运行

```bash
# 编译
cargo build --release

# 运行（DIA 模式，默认）
./target/release/cicada input.mzML --output my_track

# DDA 模式（仅 MS1）
./target/release/cicada input.mzML --output my_track --mode dda
```

## 使用说明

```
用法：cicada <INPUT> [选项]

参数：
  <INPUT>  输入 mzML 文件路径（须为质心化数据）

选项：
  -o, --output <OUTPUT>    输出文件前缀 [默认: output]
      --mode <MODE>        采集模式：dia 或 dda [默认: dia]
      --ppm <PPM>          Hill 匹配 ppm 容差 [默认: 10.0]
      --min-len <MIN_LEN>  Hill 最小数据点数 [默认: 5]
      --speed <SPEED>      时间压缩倍率（如 60.0 将 60 分钟压缩为 1 分钟）[默认: 1.0]
      --mslevel <MSLEVEL>  仅处理指定级别：1、2 或 all [默认: all]
      --start <START>      时间截取起点，单位分钟（默认：数据起始）
      --width <WIDTH>      时间截取长度，单位分钟（默认：到数据末尾）
      --no-export-hills    跳过导出 Hill CSV 文件
      --no-export-viz      跳过导出热图 PNG 和 HTML 查看器
  -h, --help               显示帮助信息
  -V, --version            显示版本信息

输出文件：
  DIA 模式：<output>_ms1.wav、<output>_ms2.wav
  DDA 模式：<output>_ms1.wav
  Hill 数据（默认开启）：<output>_ms1_hills.csv、<output>_ms2_hills.csv
  可视化（默认开启）：<output>_ms1_heatmap.png、<output>_ms1.html
                      <output>_ms2_heatmap.png、<output>_ms2.html（DIA 模式）
```

> **前提条件**：输入文件须为**质心化 (Centroided)** 的 mzML 格式。Profile 模式数据需先通过 msconvert 等工具转换。

## 许可证
[MIT](LICENSE)