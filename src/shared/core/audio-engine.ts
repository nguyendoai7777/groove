// Định nghĩa các dải tần số tiêu chuẩn theo quãng 8 (10-Band Octave Equalizer)
export const OCTAVE_BANDS = [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]

export class AudioEngine {
  private ctx: AudioContext | null = null
  private sourceNode: MediaElementAudioSourceNode | null = null
  private masterGainNode: GainNode | null = null
  private eqNodes: BiquadFilterNode[] = []
  private isInitialized = false

  constructor(private readonly audioElement: HTMLAudioElement) {
    // Ép volume của thẻ audio gốc luôn bằng 1 để nhường toàn quyền cho GainNode
    this.audioElement.volume = 1
  }

  /**
   * Khởi tạo AudioContext và cắm dây hệ thống.
   * Cần gọi hàm này sau tương tác đầu tiên của người dùng (click play) để tránh cơ chế chặn của trình duyệt.
   */
  public init(): void {
    if (this.isInitialized) return

    // 1. Khởi tạo nhà máy
    this.ctx = new (window.AudioContext || (window as any).webkitAudioContext)()

    // 2. Tạo nguồn phát từ thẻ Audio truyền vào
    this.sourceNode = this.ctx.createMediaElementSource(this.audioElement)

    // 3. Tạo các Node EQ theo quãng 8
    this.initEqualizer()

    // 4. Tạo Node quản lý âm lượng tổng
    this.masterGainNode = this.ctx.createGain()
    this.masterGainNode.gain.value = 1 // Mặc định âm lượng 100%

    // 5. CẮM DÂY (Audio Graph Pipeline)
    // Nguồn -> Filter 1 -> Filter 2 -> ... -> Filter 10 -> Master Volume -> Loa
    let lastNode: AudioNode = this.sourceNode

    for (const filter of this.eqNodes) {
      lastNode.connect(filter)
      lastNode = filter
    }

    lastNode.connect(this.masterGainNode)
    this.masterGainNode.connect(this.ctx.destination)

    this.isInitialized = true
  }

  /**
   * Tạo chuỗi 10 bộ lọc tần số
   */
  private initEqualizer(): void {
    if (!this.ctx) return

    OCTAVE_BANDS.forEach((freq, index) => {
      const filter = this.ctx!.createBiquadFilter()

      // Cấu hình kiểu lọc phù hợp cho từng dải tần
      if (index === 0) {
        filter.type = 'lowshelf' // Dải 31Hz quản lý Bass sâu nhất
      } else if (index === OCTAVE_BANDS.length - 1) {
        filter.type = 'highshelf' // Dải 16kHz quản lý Treble cao nhất
      } else {
        filter.type = 'peaking' // Các dải ở giữa (Mid)
        filter.Q.value = 1.414 // Độ rộng dải (Q) tiêu chuẩn cho bộ lọc quãng 8
      }

      filter.frequency.value = freq
      filter.gain.value = 0 // Mặc định cân bằng ở mức 0 dB

      this.eqNodes.push(filter)
    })
  }

  /**
   * Thay thế cho Audio.volume (Nhận giá trị từ 0.0 đến 1.0)
   */
  public setVolume(volume: number): void {
    this.init() // Đảm bảo đã init nếu người dùng chỉnh volume trước khi bấm play
    if (!this.masterGainNode) return

    // Giới hạn giá trị đầu vào từ 0 đến 1
    const safeVolume = Math.max(0, Math.min(1, volume))

    // Thay đổi mượt mà tránh tiếng "pực" khi vặn volume gấp
    if (this.ctx) {
      this.masterGainNode.gain.setTargetAtTime(safeVolume, this.ctx.currentTime, 0.01)
    } else {
      this.masterGainNode.gain.value = safeVolume
    }
  }

  /**
   * Lấy âm lượng hiện tại (0.0 -> 1.0)
   */
  public getVolume(): number {
    return this.masterGainNode ? this.masterGainNode.gain.value : 1
  }

  /**
   * Điều chỉnh Gain của một dải tần EQ cụ thể theo Quãng 8
   * @param bandIndex Chỉ số của dải tần (0 đến 9 tương ứng từ 31Hz đến 16kHz)
   * @param dbValue Biên độ muốn tăng/giảm, giới hạn từ -10 đến 10 dB
   */
  public setEQBand(bandIndex: number, dbValue: number): void {
    this.init()
    if (!this.eqNodes[bandIndex]) return

    // Giới hạn biên độ chạy nghiêm ngặt từ -10dB đến 10dB theo yêu cầu
    const safeDb = Math.max(-10, Math.min(10, dbValue))

    if (this.ctx) {
      // Thay đổi mượt trị số gain để tránh méo tiếng tức thời
      this.eqNodes[bandIndex].gain.setTargetAtTime(safeDb, this.ctx.currentTime, 0.01)
    } else {
      this.eqNodes[bandIndex].gain.value = safeDb
    }
  }

  /**
   * Lấy mảng giá trị hiện tại của bộ EQ (để đồng bộ lên UI Slider)
   */
  public getEQGains(): number[] {
    return this.eqNodes.map((node) => node.gain.value)
  }

  /**
   * Khởi động lại AudioContext khi bị tạm dừng bởi trình duyệt
   */
  public resume(): void {
    if (this.ctx && this.ctx.state === 'suspended') {
      this.ctx.resume().catch((err) => {
        console.error('Không thể kích hoạt lại AudioContext:', err)
      })
    }
  }
}
