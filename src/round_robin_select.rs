use core::{
    pin::Pin,
    task::{Context, Poll},
};
use defmt::info;
use embassy_futures::select::{Either3, Either4};

#[derive(Copy, Clone, Debug)]
pub enum PollFirst3 {
    A,
    B,
    C,
}
impl PollFirst3 {
    pub fn next(&mut self) {
        match self {
            PollFirst3::A => *self = PollFirst3::B,
            PollFirst3::B => *self = PollFirst3::C,
            PollFirst3::C => *self = PollFirst3::A,
        }
    }
}
pub fn round_robin_select3<A, B, C>(
    poll_first: PollFirst3,
    a: A,
    b: B,
    c: C,
) -> RoundRobinSelect3<A, B, C>
where
    A: Future,
    B: Future,
    C: Future,
{
    RoundRobinSelect3 {
        poll_first,
        a,
        b,
        c,
    }
}
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct RoundRobinSelect3<A, B, C> {
    poll_first: PollFirst3,
    a: A,
    b: B,
    c: C,
}
impl<A, B, C> Future for RoundRobinSelect3<A, B, C>
where
    A: Future,
    B: Future,
    C: Future,
{
    type Output = Either3<A::Output, B::Output, C::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let a = unsafe { Pin::new_unchecked(&mut this.a) };
        let b = unsafe { Pin::new_unchecked(&mut this.b) };
        let c = unsafe { Pin::new_unchecked(&mut this.c) };
        match this.poll_first {
            PollFirst3::A => {
                this.poll_first.next();
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either3::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either3::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either3::Third(x));
                }
            }
            PollFirst3::B => {
                this.poll_first.next();
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either3::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either3::Third(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either3::First(x));
                }
            }
            PollFirst3::C => {
                this.poll_first.next();
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either3::Third(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either3::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either3::Second(x));
                }
            }
        }
        Poll::Pending
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PollFirst4 {
    A,
    B,
    C,
    D,
}
impl PollFirst4 {
    pub fn next(&mut self) {
        match self {
            PollFirst4::A => *self = PollFirst4::B,
            PollFirst4::B => *self = PollFirst4::C,
            PollFirst4::C => *self = PollFirst4::D,
            PollFirst4::D => *self = PollFirst4::A,
        }
    }
}
pub fn round_robin_select4<A, B, C, D>(
    poll_first: PollFirst4,
    a: A,
    b: B,
    c: C,
    d: D,
) -> RoundRobinSelect4<A, B, C, D>
where
    A: Future,
    B: Future,
    C: Future,
    D: Future,
{
    RoundRobinSelect4 {
        poll_first,
        a,
        b,
        c,
        d,
    }
}
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct RoundRobinSelect4<A, B, C, D> {
    poll_first: PollFirst4,
    a: A,
    b: B,
    c: C,
    d: D,
}
impl<A, B, C, D> Future for RoundRobinSelect4<A, B, C, D>
where
    A: Future,
    B: Future,
    C: Future,
    D: Future,
{
    type Output = Either4<A::Output, B::Output, C::Output, D::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let a = unsafe { Pin::new_unchecked(&mut this.a) };
        let b = unsafe { Pin::new_unchecked(&mut this.b) };
        let c = unsafe { Pin::new_unchecked(&mut this.c) };
        let d = unsafe { Pin::new_unchecked(&mut this.d) };
        match this.poll_first {
            PollFirst4::A => {
                this.poll_first.next();
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
            }
            PollFirst4::B => {
                this.poll_first.next();
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
            }
            PollFirst4::C => {
                this.poll_first.next();
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
            }
            PollFirst4::D => {
                this.poll_first.next();
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
            }
        }
        Poll::Pending
    }
}
