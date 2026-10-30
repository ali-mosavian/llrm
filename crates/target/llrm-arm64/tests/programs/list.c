int printf(const char *, ...);
typedef unsigned long size_t;
void *malloc(size_t);
struct node { int value; struct node *next; };
static struct node *push(struct node *head, int value) {
  struct node *n = malloc(sizeof *n);
  n->value = value;
  n->next = head;
  return n;
}
static struct node *reverse(struct node *head) {
  struct node *prev = 0;
  while (head) { struct node *next = head->next; head->next = prev; prev = head; head = next; }
  return prev;
}
static int length(const struct node *n) { int c = 0; for (; n; n = n->next) c++; return c; }
struct tree { int key; struct tree *left, *right; };
static struct tree *insert(struct tree *t, int key) {
  if (!t) { t = malloc(sizeof *t); t->key = key; t->left = t->right = 0; return t; }
  if (key < t->key) t->left = insert(t->left, key); else t->right = insert(t->right, key);
  return t;
}
static void walk(const struct tree *t, int depth) {
  if (!t) return;
  walk(t->left, depth + 1);
  printf("%d@%d ", t->key, depth);
  walk(t->right, depth + 1);
}
int main(void) {
  struct node *list = 0;
  for (int i = 1; i <= 10; i++) list = push(list, i * i);
  list = reverse(list);
  int sum = 0;
  for (struct node *n = list; n; n = n->next) sum += n->value;
  printf("%d %d %d\n", length(list), sum, list->value);
  struct tree *root = 0;
  int keys[] = {50, 30, 70, 20, 40, 60, 80, 35, 45, 65};
  for (int i = 0; i < 10; i++) root = insert(root, keys[i]);
  walk(root, 0);
  printf("\n");
  return 0;
}
