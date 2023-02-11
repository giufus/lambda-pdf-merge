# variables.tf

# our service name - this will be used as a prefix for most of the resource names
variable "service_name" {
  type = string
  default = "pdf_merge"
}

# AWS region
variable "region" {
  type = string
  default = "us-east-1"
}

#API enabled
variable "default_endpoint_disabled" {
  type = bool
  default = true
}



