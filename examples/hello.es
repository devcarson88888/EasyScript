active system linux;
from library import std;

def println(a=std) {
    rules! write -> a
    rules! output -> a
}

int main() {
    println("Hello, EasyScript!");
    return code -> 0;
}
stop;
