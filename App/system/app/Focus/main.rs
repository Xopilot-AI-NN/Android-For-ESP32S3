use crate::runtime::RuntimeEvent;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Снимок рабочего этапа для отрисовки компонента задач, расходов и рабочих сессий.
pub struct FocusView {
    /// Оставшаяся длительность текущего этапа.
    pub remaining: u16,
    /// Выбранная длительность рабочего этапа в минутах.
    pub work: u8,
    /// Текущий этап конечного автомата.
    pub phase: u8,
    /// Признак выполняющейся сессии.
    pub running: bool,
    /// Количество завершённых рабочих этапов.
    pub sessions: u16,
    /// Индекс текущей цели навигации.
    pub cursor: u8,
    /// Признак окончания текущего этапа.
    pub complete: bool,
}
impl Default for FocusView {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Focus::default().view()
    }
}
/// Монотонная модель рабочего этапа и перерыва компонента задач, расходов и рабочих сессий.
pub struct Focus {
    /// Выбранная длительность рабочего этапа в минутах.
    work: u8,
    /// Текущий этап конечного автомата.
    phase: u8,
    /// Оставшаяся длительность текущего этапа.
    remaining: u16,
    /// Монотонное начало активного этапа, если он запущен.
    started: Option<u64>,
    /// Количество завершённых рабочих этапов.
    sessions: u16,
    /// Индекс текущей цели навигации.
    cursor: u8,
    /// Монотонный срок удержания внимания экрана после события.
    attention: u64,
    /// Признак окончания текущего этапа.
    complete: bool,
}
impl Default for Focus {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            work: 25,
            phase: 0,
            remaining: 1500,
            started: None,
            sessions: 0,
            cursor: 0,
            attention: 0,
            complete: false,
        }
    }
}
/// Возвращает текущий отсчёт монотонных часов.
fn now() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_secs()
}
impl Focus {
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self) {
        if self
            .started
            .is_some_and(|start| now().saturating_sub(start) >= u64::from(self.remaining))
        {
            self.started = None;
            if self.phase == 0 {
                self.sessions = self.sessions.saturating_add(1);
            }
            self.phase ^= 1;
            self.remaining = if self.phase == 0 {
                u16::from(self.work) * 60
            } else {
                300
            };
            self.complete = true;
            self.attention = now().saturating_add(5);
        }
    }
    /// Определяет, требует ли текущее состояние удержать экран включённым.
    pub fn keep_awake(&self) -> bool {
        now() < self.attention
    }
    /// Обрабатывает событие интерфейса и выполняет соответствующий переход состояния.
    pub fn event(&mut self, event: RuntimeEvent) {
        self.poll();
        match event {
            RuntimeEvent::Rotate(d) => {
                self.cursor = (i32::from(self.cursor) + d.rem_euclid(3)).rem_euclid(3) as u8
            }
            RuntimeEvent::Press => match self.cursor {
                0 => {
                    if let Some(start) = self.started.take() {
                        self.remaining =
                            self.remaining.saturating_sub(
                                now().saturating_sub(start).min(u16::MAX as u64) as u16,
                            );
                    } else {
                        self.started = Some(now());
                    }
                    self.complete = false;
                }
                1 if self.started.is_none() => {
                    self.work = match self.work {
                        15 => 25,
                        25 => 45,
                        _ => 15,
                    };
                    self.phase = 0;
                    self.remaining = u16::from(self.work) * 60;
                    self.complete = false;
                }
                2 => {
                    self.started = None;
                    self.phase = 0;
                    self.remaining = u16::from(self.work) * 60;
                    self.complete = false;
                }
                _ => {}
            },
            _ => {}
        }
    }
    /// Собирает снимок состояния для интерфейса.
    pub fn view(&self) -> FocusView {
        FocusView {
            remaining: self.started.map_or(self.remaining, |start| {
                self.remaining
                    .saturating_sub(now().saturating_sub(start).min(u16::MAX as u64) as u16)
            }),
            work: self.work,
            phase: self.phase,
            running: self.started.is_some(),
            sessions: self.sessions,
            cursor: self.cursor,
            complete: self.complete,
        }
    }
}
