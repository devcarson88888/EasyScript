active system linux;
from library import std;

def println(a=std) {
    rules! write -> a
    rules! output -> a
}

int main() {
    a = int -> 20;
    b = int -> 22;
    println(a + b);
    return code -> 0;
}
stop;
