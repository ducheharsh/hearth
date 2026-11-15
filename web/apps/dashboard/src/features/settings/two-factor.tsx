import { useEffect, useState } from "react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { ClipboardText } from "@cloudflare/kumo/components/clipboard-text";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Loader } from "@cloudflare/kumo/components/loader";
import {
  CheckCircleIcon,
  DeviceMobileIcon,
  XIcon,
} from "@phosphor-icons/react";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { useTotpStart, useTotpVerify } from "../../api/hooks";

/**
 * Enroll an authenticator app: Authelia generates the secret, we show it as a QR code,
 * and the user proves it works with one code before it counts.
 */
export function TwoFactorDialog({
  user,
  onClose,
}: {
  user: string;
  onClose: () => void;
}) {
  const start = useTotpStart();
  const verify = useTotpVerify();
  const [code, setCode] = useState("");
  const enrollment = start.data;

  useEffect(() => {
    start.mutate(user);
    // Run once per opened dialog.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [user]);

  const done = verify.isSuccess;
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="lg" className={DIALOG_FRAME}>
        <DialogFrame
          header={
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-start gap-3">
                <DeviceMobileIcon
                  size={24}
                  weight="duotone"
                  aria-hidden="true"
                  className="mt-0.5"
                />
                <div>
                  <Dialog.Title className="text-lg font-semibold">
                    Two-factor for {user}
                  </Dialog.Title>
                  <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
                    Use any authenticator app: 1Password, Google Authenticator,
                    Microsoft Authenticator, Authy, Bitwarden…
                  </Dialog.Description>
                </div>
              </div>
              <Dialog.Close
                render={(p) => (
                  <Button
                    {...p}
                    variant="ghost"
                    shape="square"
                    icon={<XIcon />}
                    aria-label="Close"
                  />
                )}
              />
            </div>
          }
          footer={
            done ? (
              <Button variant="primary" onClick={onClose}>
                Done
              </Button>
            ) : (
              <>
                <Dialog.Close
                  render={(p) => (
                    <Button {...p} variant="secondary">
                      Cancel
                    </Button>
                  )}
                />
                <Button
                  variant="primary"
                  disabled={!enrollment || code.length !== 6}
                  loading={verify.isPending}
                  onClick={() =>
                    enrollment &&
                    verify.mutate({ user, secret: enrollment.secret, code })
                  }
                >
                  Verify & Turn On
                </Button>
              </>
            )
          }
        >
          {done ? (
            <div className="flex flex-col items-center gap-3 py-6 text-center">
              <CheckCircleIcon
                size={48}
                weight="fill"
                className="text-kumo-success"
                aria-hidden="true"
              />
              <p className="font-medium">Two-factor is on.</p>
              <p className="text-sm text-kumo-subtle">
                From now on, {user} signs in with a password and a code from the
                app.
              </p>
            </div>
          ) : start.error ? (
            <Banner
              variant="error"
              title="Couldn’t start setup"
              description={start.error.message}
            />
          ) : !enrollment ? (
            <div className="flex items-center gap-2 text-sm text-kumo-subtle">
              <Loader size={16} /> Generating a secret…
            </div>
          ) : (
            <ol className="flex flex-col gap-6">
              <li className="flex flex-col gap-3">
                <p className="text-sm font-medium">
                  1. Scan this code in your authenticator app
                </p>
                <div
                  className="mx-auto w-56 rounded-xl bg-white p-3 ring-1 ring-kumo-hairline [&_svg]:h-auto [&_svg]:w-full"
                  role="img"
                  aria-label="QR code for your authenticator app"
                  // Generated server-side from a URI we built; contains no user-supplied markup.
                  dangerouslySetInnerHTML={{ __html: enrollment.qr_svg }}
                />
                <p className="text-xs text-kumo-subtle">
                  Can’t scan? Enter this key manually (time-based, 6 digits, 30
                  seconds):
                </p>
                <ClipboardText text={enrollment.secret} />
              </li>
              <li className="flex flex-col gap-3">
                <label htmlFor="totp-code" className="text-sm font-medium">
                  2. Enter the 6-digit code it shows
                </label>
                <input
                  id="totp-code"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  maxLength={6}
                  value={code}
                  onChange={(e) =>
                    setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
                  }
                  placeholder="123456"
                  className="h-14 w-full rounded-xl bg-kumo-base text-center font-mono text-3xl tracking-[0.5em] ring-1 ring-kumo-line focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kumo-brand"
                />
                {verify.error ? (
                  <p className="text-sm text-kumo-danger" role="alert">
                    {verify.error.message}
                  </p>
                ) : null}
              </li>
            </ol>
          )}
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}
