/**
 * @file 應用層與領域邏輯共用的 Result 模式定義與錯誤訊息正規化工具。
 */

/** 領域與使用案例執行失敗的語意分類。 */
export type ResultErrorType =
  | "None"
  | "Validation"
  | "NotFound"
  | "Conflict"
  | "Failure";

/**
 * 封裝用例執行結果，以結構化物件取代預期內失敗的例外拋出。
 *
 * @typeParam T 成功結果攜帶的資料型別。
 */
export class Result<T> {
  /** 代表此結果是否成功。 */
  readonly isSuccess: boolean;
  /** 成功時攜帶的回傳值，失敗時為 `null`。 */
  readonly value: T | null;
  /** 失敗時的錯誤說明訊息，成功時為 `null`。 */
  readonly error: string | null;
  /** 失敗類型的分類標籤，成功時為 `"None"`。 */
  readonly errorType: ResultErrorType;

  /**
   * 建構結構化的 `Result` 實例（僅透過靜態工廠方法建立）。
   *
   * @param isSuccess 是否成功。
   * @param value 成功結果值或 `null`。
   * @param error 失敗訊息或 `null`。
   * @param errorType 錯誤分類。
   */
  private constructor(
    isSuccess: boolean,
    value: T | null,
    error: string | null,
    errorType: ResultErrorType,
  ) {
    this.isSuccess = isSuccess;
    this.value = value;
    this.error = error;
    this.errorType = errorType;
  }

  /**
   * 建立攜帶成功值的 `Result` 實例。
   *
   * @param value 成功回傳資料。
   * @returns 代表成功的 `Result<T>` 物件。
   */
  static success<T>(value: T): Result<T> {
    return new Result<T>(true, value, null, "None");
  }

  /**
   * 建立失敗狀態的 `Result` 實例。
   *
   * @param error 失敗說明訊息。
   * @param errorType 失敗分類（預設為 `"Failure"`）。
   * @returns 代表失敗的 `Result<T>` 物件。
   */
  static failure<T = never>(
    error: string,
    errorType: Exclude<ResultErrorType, "None"> = "Failure",
  ): Result<T> {
    return new Result<T>(false, null, error, errorType);
  }

  /**
   * 建立輸入或規則驗證失敗狀態的 `Result` 實例。
   *
   * @param error 驗證錯誤提示訊息。
   * @returns `errorType` 為 `"Validation"` 的失敗 `Result<T>`。
   */
  static validation<T = never>(error: string): Result<T> {
    return Result.failure(error, "Validation");
  }
}

/**
 * 將未知型別的攔截例外正規化為可讀的錯誤字串。
 *
 * @param reason `catch` 攔截到的任意錯誤值。
 * @returns 若為 `Error` 實例則回傳 `reason.message`，否則轉為字串。
 */
export function normalizeErrorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}
