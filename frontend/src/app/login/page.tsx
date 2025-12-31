"use client";

import { AlertCircle } from "lucide-react";
import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { signIn, signUp } from "@/lib/auth-client";

function LoginForm() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [isSignUp, setIsSignUp] = useState(false);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [registrationAllowed, setRegistrationAllowed] = useState(true);
  const [checkingRegistration, setCheckingRegistration] = useState(false);
  const registrationToken = searchParams.get("token");

  useEffect(() => {
    const checkRegistrationStatus = async () => {
      if (!isSignUp) return;
      
      setCheckingRegistration(true);
      try {
        const apiUrl = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";
        const url = new URL(`${apiUrl}/api/auth/registration-status`);
        if (registrationToken) {
          url.searchParams.set("token", registrationToken);
        }
        
        const response = await fetch(url.toString());
        const data = await response.json();
        
        if (!data.allowed) {
          setError("Registration is disabled. A valid registration token is required.");
          setRegistrationAllowed(false);
          setIsSignUp(false);
        } else {
          setRegistrationAllowed(true);
          setError("");
        }
      } catch (err) {
        console.error("Failed to check registration status:", err);
      } finally {
        setCheckingRegistration(false);
      }
    };

    checkRegistrationStatus();
  }, [isSignUp, registrationToken]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);

    try {
      if (isSignUp) {
        if (!registrationAllowed) {
          setError("Registration is not allowed without a valid token.");
          return;
        }
        
        const result = await signUp.email({
          email,
          password,
          name,
          callbackURL: "/",
        });

        if (result.error) {
          setError(result.error.message || "Sign up failed. Please try again.");
          return;
        }

        router.push("/projects");
      } else {
        const result = await signIn.email({
          email,
          password,
        });

        if (result.error) {
          const errorMsg = result.error.message || "";
          if (
            errorMsg.includes("Invalid") ||
            errorMsg.includes("credentials") ||
            errorMsg.includes("password")
          ) {
            setError("Invalid email or password. Please try again.");
          } else if (
            errorMsg.includes("not found") ||
            errorMsg.includes("exist")
          ) {
            setError("No account found with this email. Please sign up.");
          } else {
            setError(
              "Sign in failed. Please check your credentials and try again.",
            );
          }
          return;
        }

        router.push("/");
      }
    } catch (err: unknown) {
      console.error("Auth error:", err);
      if (isSignUp) {
        setError("Sign up failed. The email may already be in use.");
      } else {
        setError("Invalid email or password. Please try again.");
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <Card className="w-full max-w-md">
        <CardHeader className="space-y-1 py-8">
          <CardTitle className="text-3xl font-bold text-center">iLog</CardTitle>
          <CardDescription className="text-center">
            Centralized logging solution.
          </CardDescription>
        </CardHeader>
        <CardContent className="pb-8">
          <form onSubmit={handleSubmit} className="space-y-4">
            {isSignUp && (
              <div className="space-y-2">
                <Label htmlFor="name">Name</Label>
                <Input
                  id="name"
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required={isSignUp}
                  placeholder="Enter your name"
                />
              </div>
            )}

            <div className="space-y-2">
              <Label htmlFor="email">Email</Label>
              <Input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
                placeholder="Enter your email"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="password">Password</Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                placeholder="Enter your password"
              />
            </div>

            {error && (
              <Alert variant="destructive">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}

            <Button type="submit" disabled={loading} className="w-full">
              {loading ? "Loading..." : isSignUp ? "Sign Up" : "Sign In"}
            </Button>
          </form>

          <div className="mt-6 text-center">
            <Button
              variant="link"
              onClick={() => {
                if (!isSignUp) {
                  setIsSignUp(true);
                } else {
                  setIsSignUp(false);
                  setError("");
                }
              }}
              className="text-sm"
              disabled={checkingRegistration}
            >
              {isSignUp
                ? "Already have an account? Sign in"
                : "Don't have an account? Sign up"}
            </Button>
            {registrationToken && isSignUp && (
              <p className="text-xs text-muted-foreground mt-2">
                Using registration token
              </p>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

export default function LoginPage() {
  return (
    <Suspense fallback={
      <div className="min-h-screen flex items-center justify-center bg-background">
        <Card className="w-full max-w-md">
          <CardHeader className="space-y-1 py-8">
            <CardTitle className="text-3xl font-bold text-center">iLog</CardTitle>
            <CardDescription className="text-center">
              Loading...
            </CardDescription>
          </CardHeader>
        </Card>
      </div>
    }>
      <LoginForm />
    </Suspense>
  );
}
