terraform {

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 4.0"
    }
  }

  required_version = ">= 1.3.7"

}

# Configure the AWS Provider
provider "aws" {
  region = var.region
}

resource "random_pet" "lambda_random_name" {
  prefix = var.service_name
  length = 4

  provisioner "local-exec" {
    command = "cd ..; cargo lambda build --release --arm64 --output-format zip"
  }
}

resource "aws_iam_role" "iam_for_lambda" {
  name = "iam_for_lambda-${terraform.workspace}-${var.service_name}"

  assume_role_policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Action": "sts:AssumeRole",
      "Principal": {
        "Service": "lambda.amazonaws.com"
      },
      "Effect": "Allow",
      "Sid": ""
    }
  ]
}
EOF
}

resource "aws_iam_policy" "iam_policy_for_lambda" {

  name         = "aws_iam_policy_for_terraform_${terraform.workspace}-${var.service_name}"
  path         = "/"
  description  = "AWS IAM Policy for managing aws lambda role"
  policy = <<EOF
{
 "Version": "2012-10-17",
 "Statement": [
   {
     "Action": [
       "logs:CreateLogGroup",
       "logs:CreateLogStream",
       "logs:PutLogEvents"
     ],
     "Resource": "arn:aws:logs:*:*:*",
     "Effect": "Allow"
   }
 ]
}
EOF
}

resource "aws_iam_role_policy_attachment" "attach_iam_policy_to_iam_role" {
  role        = aws_iam_role.iam_for_lambda.name
  policy_arn  = aws_iam_policy.iam_policy_for_lambda.arn
}

resource "aws_lambda_function" "lambda_function" {
  # If the file is not in the current working directory you will need to include a
  # path.module in the filename.
  filename      = "../target/lambda/lambda-pdf-merge/bootstrap.zip"
  function_name = "${terraform.workspace}-${var.service_name}_lambda_function"
  role          = aws_iam_role.iam_for_lambda.arn
  handler       = "bootstrap"
  depends_on    = [aws_iam_role_policy_attachment.attach_iam_policy_to_iam_role]
  architectures = ["arm64"]


  # The filebase64sha256() function is available in Terraform 0.11.12 and later
  # For Terraform 0.11.11 and earlier, use the base64sha256() function and the file() function:
  # source_code_hash = "${base64sha256(file("lambda_function_payload.zip"))}"
  source_code_hash = filebase64sha256("../target/lambda/lambda-pdf-merge/bootstrap.zip")

  runtime = "provided.al2"
}

# HTTP API
resource "aws_apigatewayv2_api" "api" {
  name          = "${terraform.workspace}_api-pdf-merge"
  protocol_type = "HTTP"
  target        = aws_lambda_function.lambda_function.arn
}

# Permission
resource "aws_lambda_permission" "apigw" {
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.lambda_function.arn
  principal     = "apigateway.amazonaws.com"

  source_arn = "${aws_apigatewayv2_api.api.execution_arn}/*/*"
}



