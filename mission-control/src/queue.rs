const QUEUE_SIZE: u8 = 16;

pub struct Queue<T> {
    q_first: u8,
    q_last: u8,
    queue_array: [Option<T>; 16],
}

impl<T> Queue<T> {
    pub fn new() -> Self {
        return Queue {
            q_first: 0,
            q_last: 0,
            queue_array: std::array::from_fn(|_| None),
        };
    }

    pub fn add(&mut self, item: T) {
        self.queue_array[usize::from(self.q_last)] = Some(item);
        self.q_last = (self.q_last + 1) % QUEUE_SIZE;
    }

    pub fn remove(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let item = self.queue_array[usize::from(self.q_first)].take();
        self.q_first = (self.q_first + 1) % QUEUE_SIZE;
        return item;
    }

    pub fn is_empty(&self) -> bool {
        return self.q_first == self.q_last;
    }
}
