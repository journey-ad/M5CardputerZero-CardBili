#include <stdio.h>
#include <stdlib.h>

int ffmpeg_main(int argc, char **argv);

int main(int argc, char **argv)
{
    if (argc > 1)
        return ffmpeg_main(argc, argv);

    FILE *input = fdopen(4, "rb");
    char **args = calloc(1, sizeof(*args));
    int count = 1;
    int result = 1;
    char *value = NULL;
    size_t capacity = 0;
    ssize_t length;
    if (!input || !args)
        goto finish;
    args[0] = argv[0];

    // 参数以 NUL 分隔，管道关闭表示参数传输结束
    while ((length = getdelim(&value, &capacity, '\0', input)) > 0) {
        if (value[length - 1] != '\0')
            goto finish;
        char **grown = realloc(args, (count + 1) * sizeof(*args));
        if (!grown)
            goto finish;
        args = grown;
        args[count++] = value;
        value = NULL;
        capacity = 0;
    }
    if (ferror(input))
        goto finish;
    char **grown = realloc(args, (count + 1) * sizeof(*args));
    if (!grown)
        goto finish;
    args = grown;
    args[count] = NULL;
    fclose(input);
    input = NULL;
    result = ffmpeg_main(count, args);

finish:
    if (input)
        fclose(input);
    free(value);
    for (int i = 1; i < count; i++)
        free(args[i]);
    free(args);
    return result;
}
