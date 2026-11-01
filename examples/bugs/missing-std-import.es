int main() {
    rules! write -> "This should fail without the std import";
    rules! output -> a;
    return code -> 0;
}
