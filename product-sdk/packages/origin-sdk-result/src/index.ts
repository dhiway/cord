// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

export type Ok<T> = Readonly<{ success: true; value: T }>;
export type Err<E> = Readonly<{ success: false; error: E }>;
export type Result<T, E> = Ok<T> | Err<E>;

export const ok = <T>(value: T): Ok<T> => ({ success: true, value });
export const err = <E>(error: E): Err<E> => ({ success: false, error });
export const isOk = <T, E>(result: Result<T, E>): result is Ok<T> => result.success;
export const isErr = <T, E>(result: Result<T, E>): result is Err<E> => !result.success;

export function map<T, U, E>(result: Result<T, E>, transform: (value: T) => U): Result<U, E> {
  return result.success ? ok(transform(result.value)) : result;
}

export function mapError<T, E, F>(result: Result<T, E>, transform: (error: E) => F): Result<T, F> {
  return result.success ? result : err(transform(result.error));
}

export function andThen<T, U, E>(
  result: Result<T, E>,
  transform: (value: T) => Result<U, E>,
): Result<U, E> {
  return result.success ? transform(result.value) : result;
}

export const unwrapOr = <T, E>(result: Result<T, E>, fallback: T): T =>
  result.success ? result.value : fallback;
