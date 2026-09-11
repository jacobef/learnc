void addscalar(int n, int m, double a[n][n * m + 300], double x);

int main(void) {
    double b[4][308] = { 0 };
    addscalar(4, 2, b, 2.17);
    return b[3][307] != 2.17;
}

void addscalar(int n, int m, double a[n][n * m + 300], double x) {
    for (int i = 0; i < n; i++)
        for (int j = 0; j < n * m + 300; j++) a[i][j] += x;
}
