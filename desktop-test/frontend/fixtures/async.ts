/**
 * @file async.ts
 * @description 提供前端非同步測試使用的延遲 Promise 控制工具（Deferred helper）。
 */

/**
 * 建立可由外部手動控制完成（resolve）與失敗（reject）時機的非同步 Promise 容器。
 *
 * @template T Promise 解析後的資料型別。
 * @returns 包含 `promise` 實例與對應 `resolve`、`reject` 控制函式的物件。
 */
export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
