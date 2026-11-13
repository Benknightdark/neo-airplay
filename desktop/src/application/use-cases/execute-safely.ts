/**
 * @file 應用層共用執行包裝器，將非同步閘道呼叫捕捉到的例外轉換為結構化 `Result` 物件。
 */
import { normalizeErrorMessage, Result } from "../../domain";

/**
 * 安全執行非同步使用案例作業，避免表現層直接處理未捕獲例外。
 *
 * @typeParam T 成功結果的資料型別。
 * @param operation 欲執行的非同步閘道操作閉包。
 * @returns 成功時回傳 `Result.success(value)`；捕捉到異常時回傳 `Result.failure(message)`。
 */
export async function executeSafely<T>(operation: () => Promise<T>): Promise<Result<T>> {
  try {
    return Result.success(await operation());
  } catch (reason) {
    return Result.failure(normalizeErrorMessage(reason));
  }
}
